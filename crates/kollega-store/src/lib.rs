//! Persistance PostgreSQL — le point de passage **unique** vers la base.
//!
//! Invariant 1 : l'isolation est appliquée par la base (Row-Level Security),
//! et chaque transaction commence par `SET LOCAL app.current_org`. Ce module
//! est le seul chemin d'accès aux données : le pool est privé, et toute
//! transaction s'ouvre par [`Db::org_transaction`], qui pose le contexte
//! d'organisation avant toute requête. C'est ce point unique qu'on audite
//! et qu'on teste.

#![forbid(unsafe_code)]

pub mod driver;

use sqlx::postgres::{PgPool, PgPoolOptions};
use sqlx::{Postgres, Transaction};
use uuid::Uuid;

/// Erreurs de la couche de persistance.
#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    /// Erreur PostgreSQL (connexion, requête).
    #[error("erreur PostgreSQL : {0}")]
    Database(#[from] sqlx::Error),
    /// Erreur d'application des migrations.
    #[error("erreur de migration : {0}")]
    Migration(#[from] sqlx::migrate::MigrateError),
    /// Tâche introuvable dans le contexte d'organisation courant.
    #[error("tâche introuvable dans ce contexte d'organisation")]
    TaskNotFound,
    /// Donnée persistée illisible ou incohérente — à traiter, pas à deviner.
    #[error("état persistant corrompu : {0}")]
    CorruptState(String),
    /// La vérification de la chaîne d'audit a trouvé une rupture.
    #[error("chaîne d'audit invalide : {0}")]
    ChainBroken(String),
    /// Erreur comptable (budget) au rechargement ou à l'amorçage.
    #[error("erreur comptable : {0}")]
    Accounting(String),
    /// Un autre écrivain a pris la hauteur de chaîne visée : le pas entier
    /// est à rejouer (violation d'unicité sur `(org_id, height)`).
    #[error("conflit d'écriture de chaîne : hauteur prise, pas à rejouer")]
    ChainConflict,
}

/// Accès à la base pour l'application (rôle `kollega_app`, sans `BYPASSRLS`).
///
/// Le pool est privé : aucun autre chemin d'accès aux données ne doit exister
/// dans le code. Les données des tenants ne sont accessibles que via
/// [`Db::org_transaction`].
#[derive(Clone)]
pub struct Db {
    pool: PgPool,
}

impl Db {
    /// Ouvre le pool de connexions de l'application.
    pub async fn connect(database_url: &str) -> Result<Self, StoreError> {
        let pool = PgPoolOptions::new()
            .max_connections(5)
            .connect(database_url)
            .await?;
        Ok(Db { pool })
    }

    /// Vérifie que la base répond : un `SELECT 1` doit aboutir.
    ///
    /// Seul accès autorisé sans contexte d'organisation — il ne touche
    /// aucune table.
    pub async fn health(&self) -> Result<(), StoreError> {
        sqlx::query("SELECT 1").execute(&self.pool).await?;
        Ok(())
    }

    /// Ouvre une transaction dans le contexte d'une organisation.
    ///
    /// Exécute `SET LOCAL app.current_org = <org_id>` (via `set_config(...,
    /// true)`, la forme paramétrable) avant toute requête. Le réglage est
    /// local à la transaction : il disparaît au commit comme au rollback, et
    /// une requête émise hors de ce contexte échoue (politique RLS fermée
    /// par défaut).
    pub async fn org_transaction(
        &self,
        org_id: Uuid,
    ) -> Result<Transaction<'static, Postgres>, StoreError> {
        let mut tx = self.pool.begin().await?;
        sqlx::query("SELECT set_config('app.current_org', $1, true)")
            .bind(org_id.to_string())
            .execute(&mut *tx)
            .await?;
        Ok(tx)
    }
}

/// Applique les migrations embarquées, avec le rôle `kollega_migrate`.
///
/// Connexion distincte de celle de l'application : les migrations possèdent le
/// schéma, l'application ne le possède jamais.
pub async fn run_migrations(migrate_database_url: &str) -> Result<(), StoreError> {
    let pool = PgPoolOptions::new()
        .max_connections(1)
        .connect(migrate_database_url)
        .await?;
    sqlx::migrate!("../../migrations").run(&pool).await?;
    Ok(())
}

/// Pose le mot de passe du rôle applicatif `kollega_app`.
///
/// Appelé par `kollega migrate` quand `KOLLEGA_APP_DB_PASSWORD` est défini :
/// le mot de passe vient de l'environnement de l'exploitant, jamais d'une
/// migration (aucun secret dans l'historique). `ALTER ROLE` n'accepte pas de
/// paramètre lié : le littéral est construit en doublant les apostrophes.
///
/// La journalisation d'énoncés de sqlx est coupée sur cette connexion (y
/// compris le chemin « requête lente » à WARN) : le secret ne doit apparaître
/// dans aucun journal client. Risque résiduel côté serveur : avec
/// `log_statement='ddl'` ou en cas d'échec (`log_min_error_statement`),
/// PostgreSQL journalise l'énoncé — l'exploitant garde les valeurs par défaut.
pub async fn set_app_role_password(
    migrate_database_url: &str,
    password: &str,
) -> Result<(), StoreError> {
    use sqlx::postgres::PgConnectOptions;
    use sqlx::ConnectOptions;
    use std::str::FromStr;

    let options = PgConnectOptions::from_str(migrate_database_url)?
        .log_statements(log::LevelFilter::Off)
        .log_slow_statements(log::LevelFilter::Off, std::time::Duration::ZERO);
    let mut conn = options.connect().await?;
    let literal = password.replace('\'', "''");
    let statement = format!("ALTER ROLE kollega_app WITH LOGIN PASSWORD '{literal}'");
    sqlx::query(&statement).execute(&mut conn).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn crate_compiles() {}
}
