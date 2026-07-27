//! Invariant 12 : aucune suppression physique — porté par le RÔLE.
//!
//! « Effacement logique, plus purge et export explicites par organisation,
//! tracés. » Tant que le rôle applicatif pouvait exécuter un `DELETE`, la
//! règle reposait sur la relecture en revue. Ce test la met à l'épreuve :
//! le rôle tente de supprimer dans CHAQUE table tenant, et doit échouer
//! partout — sauf sur le contenu d'audit, seule suppression légitime, qui
//! est la purge RGPD et passe par un acte nommé laissant une attestation.
//!
//! Exige `TEST_MIGRATE_DATABASE_URL` (fournie en CI) ; sauté sinon.

use kollega_core::Cents;
use kollega_store::driver;
use sqlx::{Connection, PgConnection, Row as _};
use uuid::Uuid;

const APP_PASSWORD: &str = "kollega_app_test_pw";

fn app_url_from(migrate_url: &str) -> String {
    use sqlx::postgres::PgConnectOptions;
    use std::str::FromStr;
    let opts = PgConnectOptions::from_str(migrate_url).expect("URL invalide");
    format!(
        "postgres://kollega_app:{APP_PASSWORD}@{}:{}/{}",
        opts.get_host(),
        opts.get_port(),
        opts.get_database().unwrap_or("kollega"),
    )
}

#[tokio::test]
async fn the_application_role_cannot_physically_delete_anything_but_purgeable_content() {
    let Ok(migrate_url) = std::env::var("TEST_MIGRATE_DATABASE_URL") else {
        eprintln!("IGNORÉ : TEST_MIGRATE_DATABASE_URL absent — exécuté en CI.");
        return;
    };

    kollega_store::run_migrations(&migrate_url)
        .await
        .expect("migrations");
    kollega_store::set_app_role_password(&migrate_url, APP_PASSWORD)
        .await
        .expect("mot de passe kollega_app");

    let org = Uuid::from_u128(0xDE_1E7E);
    let mut admin = PgConnection::connect(&migrate_url).await.expect("admin");
    for table in [
        "audit_content",
        "audit_chain",
        "tool_call_effects",
        "tasks",
        "credits",
        "users",
    ] {
        sqlx::query(&format!("DELETE FROM {table} WHERE org_id = $1"))
            .bind(org)
            .execute(&mut admin)
            .await
            .expect("nettoyage");
    }
    sqlx::query("DELETE FROM organizations WHERE id = $1")
        .bind(org)
        .execute(&mut admin)
        .await
        .expect("nettoyage org");

    let db = kollega_store::Db::connect(&app_url_from(&migrate_url))
        .await
        .expect("connexion");

    // Un peu de matière à supprimer : une organisation, un utilisateur, un
    // crédit, une tâche — et des attestations produites par un vrai pas.
    let mut tx = db.org_transaction(org).await.expect("tx seed");
    sqlx::query("INSERT INTO organizations (id, name) VALUES ($1, 'Org Effacement')")
        .bind(org)
        .execute(&mut *tx)
        .await
        .expect("org");
    sqlx::query(
        "INSERT INTO users (id, org_id, email, role) VALUES ($1, $2, 'a@b.example', 'proprietaire')",
    )
    .bind(Uuid::new_v4())
    .bind(org)
    .execute(&mut *tx)
    .await
    .expect("utilisateur");
    sqlx::query("INSERT INTO credits (org_id, balance_cents) VALUES ($1, 1000)")
        .bind(org)
        .execute(&mut *tx)
        .await
        .expect("crédit");
    tx.commit().await.expect("commit seed");

    let task = Uuid::new_v4();
    driver::create_task(&db, org, task, Cents(500), 4)
        .await
        .expect("création de tâche");

    // Toute tentative de suppression physique par le rôle applicatif
    // ÉCHOUE — sur chaque table tenant, sans exception.
    for table in [
        "organizations",
        "users",
        "credits",
        "tasks",
        "audit_chain",
        "tool_call_effects",
    ] {
        let mut tx = db.org_transaction(org).await.expect("tx suppression");
        let attempt = sqlx::query(&format!("DELETE FROM {table}"))
            .execute(&mut *tx)
            .await;
        assert!(
            attempt.is_err(),
            "kollega_app ne doit pas pouvoir supprimer dans {table} — \
             l'effacement est LOGIQUE (deleted_at), jamais physique"
        );
        drop(tx);
    }

    // La seule exception, et elle porte son nom : le contenu d'audit est
    // purgeable (RGPD), par un acte tracé qui laisse la chaîne vérifiable.
    let purged = driver::purge_org_content(&db, org)
        .await
        .expect("la purge RGPD, elle, est permise");
    let check = driver::verify_org_chain(&db, org)
        .await
        .expect("la chaîne survit à la purge");
    assert!(
        check.entries > 0,
        "purger le contenu ne retire aucune attestation ({purged} contenus purgés)"
    );

    // Et la tâche est toujours là : rien n'a été effacé physiquement.
    let mut tx = db.org_transaction(org).await.expect("tx vérification");
    let remaining: i64 = sqlx::query("SELECT count(*) FROM tasks")
        .fetch_one(&mut *tx)
        .await
        .expect("comptage")
        .get(0);
    assert_eq!(remaining, 1, "la tâche n'a pas pu être supprimée");
}
