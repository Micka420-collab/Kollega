//! Test d'isolation multi-tenant (invariant 1) — la seule preuve qui compte.
//!
//! Nécessite une base PostgreSQL réelle avec pgvector, désignée par la
//! variable d'environnement `TEST_MIGRATE_DATABASE_URL` (rôle capable
//! d'appliquer les migrations — en CI, le superutilisateur du conteneur).
//! Sans elle, le test est sauté avec un message explicite : il s'exécute en
//! intégration continue, où le service PostgreSQL est provisionné.
//!
//! Déroulé :
//! 1. migrations appliquées, mot de passe du rôle `kollega_app` posé ;
//! 2. données insérées pour deux organisations A et B via le point de passage
//!    unique (`Db::org_transaction`) ;
//! 3. dans le contexte de A, un SELECT sans clause WHERE ne voit QUE A ;
//! 4. hors de tout contexte, la requête ÉCHOUE (fermé par défaut) ;
//! 5. sensibilité : RLS désactivée à la main => la fuite devient visible —
//!    preuve que ce test échouerait si la RLS tombait — puis RLS réactivée.

use sqlx::{Connection, PgConnection, Row};
use uuid::Uuid;

const APP_PASSWORD: &str = "kollega_app_test_pw";

fn migrate_url() -> Option<String> {
    std::env::var("TEST_MIGRATE_DATABASE_URL").ok()
}

fn app_url_from(migrate_url: &str) -> String {
    use sqlx::postgres::PgConnectOptions;
    use std::str::FromStr;
    let opts = PgConnectOptions::from_str(migrate_url).expect("TEST_MIGRATE_DATABASE_URL invalide");
    format!(
        "postgres://kollega_app:{APP_PASSWORD}@{}:{}/{}",
        opts.get_host(),
        opts.get_port(),
        opts.get_database().unwrap_or("kollega"),
    )
}

async fn count(conn: &mut PgConnection, sql: &str) -> i64 {
    sqlx::query(sql)
        .fetch_one(conn)
        .await
        .expect("requête de comptage")
        .get::<i64, _>(0)
}

#[tokio::test]
async fn tenant_isolation_holds_and_the_test_is_sensitive() {
    let Some(migrate_url) = migrate_url() else {
        eprintln!(
            "IGNORÉ : TEST_MIGRATE_DATABASE_URL absent — le test d'isolation \
             exige une base réelle (exécuté en CI)."
        );
        return;
    };

    // 1. Migrations, mot de passe applicatif, base propre.
    kollega_store::run_migrations(&migrate_url)
        .await
        .expect("migrations");
    kollega_store::set_app_role_password(&migrate_url, APP_PASSWORD)
        .await
        .expect("mot de passe kollega_app");

    let mut admin = PgConnection::connect(&migrate_url).await.expect("admin");
    // Le nettoyage se fait avec le rôle de migration (superutilisateur en CI),
    // qui n'est pas soumis à la RLS.
    sqlx::query("TRUNCATE users, organizations")
        .execute(&mut admin)
        .await
        .expect("nettoyage");

    // 2. Deux organisations, insérées par le point de passage unique.
    let db = kollega_store::Db::connect(&app_url_from(&migrate_url))
        .await
        .expect("connexion kollega_app");
    let org_a = Uuid::from_u128(0xA);
    let org_b = Uuid::from_u128(0xB);
    for (org, name, email) in [
        (org_a, "Org A", "a@a.example"),
        (org_b, "Org B", "b@b.example"),
    ] {
        let mut tx = db.org_transaction(org).await.expect("transaction org");
        sqlx::query("INSERT INTO organizations (id, name) VALUES ($1, $2)")
            .bind(org)
            .bind(name)
            .execute(&mut *tx)
            .await
            .expect("insertion organisation");
        sqlx::query(
            "INSERT INTO users (id, org_id, email, role) VALUES ($1, $2, $3, 'proprietaire')",
        )
        .bind(Uuid::new_v4())
        .bind(org)
        .bind(email)
        .execute(&mut *tx)
        .await
        .expect("insertion utilisateur");
        tx.commit().await.expect("commit");
    }

    // Témoin : vu du rôle de migration, les deux organisations existent bien.
    // Sans ce témoin, « zéro fuite » pourrait n'être que « zéro donnée ».
    assert_eq!(
        count(&mut admin, "SELECT count(*) FROM organizations").await,
        2
    );

    // 3. Dans le contexte de A : uniquement les lignes de A, sans clause WHERE.
    let mut tx = db.org_transaction(org_a).await.expect("transaction A");
    let orgs: Vec<Uuid> = sqlx::query("SELECT id FROM organizations")
        .fetch_all(&mut *tx)
        .await
        .expect("SELECT organizations")
        .into_iter()
        .map(|r| r.get::<Uuid, _>(0))
        .collect();
    assert_eq!(orgs, vec![org_a], "le contexte de A ne doit voir que A");
    let users_orgs: Vec<Uuid> = sqlx::query("SELECT org_id FROM users")
        .fetch_all(&mut *tx)
        .await
        .expect("SELECT users")
        .into_iter()
        .map(|r| r.get::<Uuid, _>(0))
        .collect();
    assert_eq!(users_orgs, vec![org_a], "aucun utilisateur d'une autre org");
    drop(tx);

    // 4. Hors contexte : la requête échoue — fermé par défaut, pas silencieux.
    let mut plain = PgConnection::connect(&app_url_from(&migrate_url))
        .await
        .expect("connexion sans contexte");
    let out_of_context = sqlx::query("SELECT count(*) FROM users")
        .fetch_one(&mut plain)
        .await;
    assert!(
        out_of_context.is_err(),
        "sans app.current_org, la requête doit échouer, pas retourner des lignes"
    );

    // 5. Sensibilité : si la RLS tombe, la fuite doit devenir visible —
    //    c'est la preuve que ce test échouerait en cas de régression.
    sqlx::query("ALTER TABLE users DISABLE ROW LEVEL SECURITY")
        .execute(&mut admin)
        .await
        .expect("désactivation RLS");
    let mut tx = db.org_transaction(org_a).await.expect("transaction A");
    let leaked = count(&mut tx, "SELECT count(*) FROM users").await;
    drop(tx);
    assert_eq!(
        leaked, 2,
        "RLS désactivée, la fuite doit apparaître — sinon ce test ne prouve rien"
    );

    // Restauration, puis re-vérification de l'isolation.
    sqlx::query("ALTER TABLE users ENABLE ROW LEVEL SECURITY")
        .execute(&mut admin)
        .await
        .expect("réactivation RLS");
    sqlx::query("ALTER TABLE users FORCE ROW LEVEL SECURITY")
        .execute(&mut admin)
        .await
        .expect("FORCE RLS");
    let mut tx = db.org_transaction(org_a).await.expect("transaction A");
    assert_eq!(count(&mut tx, "SELECT count(*) FROM users").await, 1);
}
