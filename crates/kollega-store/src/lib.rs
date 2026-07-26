//! Persistance PostgreSQL. Au jalon M0 : connexion et migrations, rien d'autre.

#![forbid(unsafe_code)]

use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;

/// Ouvre un pool de connexions vers PostgreSQL.
pub async fn connect(database_url: &str) -> Result<PgPool, sqlx::Error> {
    PgPoolOptions::new()
        .max_connections(5)
        .connect(database_url)
        .await
}

/// Applique les migrations du dossier `migrations/` à la racine du dépôt.
/// Les fichiers sont embarqués dans le binaire à la compilation.
pub async fn run_migrations(pool: &PgPool) -> Result<(), sqlx::migrate::MigrateError> {
    sqlx::migrate!("../../migrations").run(pool).await
}

#[cfg(test)]
mod tests {
    #[test]
    fn crate_compiles() {}
}
