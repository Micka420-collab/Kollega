//! Le validateur de séquence sait DIRE NON sur des données réelles.
//!
//! `vertical_slice` prouve qu'il approuve une chaîne saine ; c'est
//! nécessaire, mais insuffisant : un validateur qui dirait toujours oui
//! passerait ce test-là. Ici, on injecte en base les violations que la
//! table du bloc 3e déclare interdites, et l'on vérifie qu'il les nomme.
//!
//! Le rôle applicatif PEUT insérer dans `audit_chain` (ajout seul) : c'est
//! donc un scénario atteignable, pas une manipulation de laboratoire — un
//! bug d'écriture produirait exactement cela.
//!
//! Exige `TEST_MIGRATE_DATABASE_URL` (fournie en CI) ; sauté sinon.

use kollega_audit::records::ViolationKind;
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

/// Insère une attestation brute à la hauteur suivante. Les hachages ne sont
/// pas cohérents — sans importance ici : la vérification de SÉQUENCE ne
/// regarde pas la chaîne, elle regarde ce que les attestations racontent.
async fn insert_raw(
    db: &kollega_store::Db,
    org: Uuid,
    action: &str,
    tool_call_id: Uuid,
) -> Result<(), sqlx::Error> {
    let mut tx = db.org_transaction(org).await.expect("tx");
    let height: i64 = sqlx::query("SELECT COALESCE(MAX(height), -1) + 1 FROM audit_chain")
        .fetch_one(&mut *tx)
        .await?
        .get(0);
    sqlx::query(
        "INSERT INTO audit_chain \
         (org_id, height, prev_hash, entry_hash, actor, action, tool_call_id, \
          content_digest, timestamp_micros) \
         VALUES ($1, $2, NULL, $3, 'test', $4, $5, $3, 0)",
    )
    .bind(org)
    .bind(height)
    .bind(vec![0u8; 32])
    .bind(action)
    .bind(tool_call_id)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(())
}

#[tokio::test]
async fn the_sequence_validator_names_every_forbidden_shape() {
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

    let org = Uuid::from_u128(0x5E_9E_11);
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
    let mut tx = db.org_transaction(org).await.expect("tx seed");
    sqlx::query("INSERT INTO organizations (id, name) VALUES ($1, 'Org Séquence')")
        .bind(org)
        .execute(&mut *tx)
        .await
        .expect("org");
    tx.commit().await.expect("commit seed");

    // --- 1. Clôture SANS intention ------------------------------------------
    let orphan = Uuid::from_u128(0xA1);
    insert_raw(&db, org, "tool_call_completed", orphan)
        .await
        .expect("insertion");
    let report = driver::verify_org_sequence(&db, org)
        .await
        .expect("lecture");
    assert!(
        !report.is_valid(),
        "une clôture orpheline doit être dénoncée"
    );
    assert_eq!(
        report.violations[0].kind,
        ViolationKind::ClosureWithoutIntent,
        "et la violation doit être NOMMÉE, pas seulement comptée"
    );

    // --- 2. Double intention ------------------------------------------------
    let twice = Uuid::from_u128(0xB2);
    insert_raw(&db, org, "tool_call_intended", twice)
        .await
        .expect("insertion");
    insert_raw(&db, org, "tool_call_intended", twice)
        .await
        .expect("insertion");
    let report = driver::verify_org_sequence(&db, org)
        .await
        .expect("lecture");
    assert!(
        report
            .violations
            .iter()
            .any(|v| v.kind == ViolationKind::DuplicateIntent),
        "une seconde intention pour le même appel est une violation : {:?}",
        report.violations
    );

    // --- 3. Un appel OUVERT n'est pas une violation -------------------------
    // L'asymétrie du bloc 3e, sur données réelles : l'appel `twice` reste
    // ouvert, et il est rapporté comme tel — information, pas faute.
    assert!(
        report
            .open_calls
            .contains(&kollega_core::ToolCallId::new(twice)),
        "l'appel resté ouvert doit être SIGNALÉ : {:?}",
        report.open_calls
    );

    // --- 4. Un enregistrement APRÈS clôture ---------------------------------
    let closed = Uuid::from_u128(0xC3);
    insert_raw(&db, org, "tool_call_intended", closed)
        .await
        .expect("insertion");
    insert_raw(&db, org, "tool_call_completed", closed)
        .await
        .expect("insertion");
    insert_raw(&db, org, "tool_call_denied", closed)
        .await
        .expect("insertion");
    let report = driver::verify_org_sequence(&db, org)
        .await
        .expect("lecture");
    assert!(
        report
            .violations
            .iter()
            .any(|v| v.kind == ViolationKind::DuplicateClosure),
        "une seconde clôture est une violation : {:?}",
        report.violations
    );

    // Les positions rapportées permettent de RETROUVER la ligne fautive :
    // un rapport qui dirait seulement « invalide » n'aiderait personne.
    assert!(
        report.violations.iter().all(|v| v.position < 10),
        "chaque violation porte sa position dans la séquence"
    );
}
