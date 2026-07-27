//! `kollega audit verify` dit vrai — DANS LES DEUX SENS.
//!
//! Une commande de vérification qui répondrait toujours « intègre » serait
//! pire qu'inexistante : elle donnerait une confiance imméritée. Ce test
//! l'exécute pour de vrai, sur une chaîne saine (code 0) PUIS sur la même
//! chaîne corrompue (code 1, message explicite).
//!
//! C'est le dernier maillon de l'invariant 4 : la vérification existait en
//! bibliothèque, mais aucun exploitant ne pouvait la lancer.
//!
//! Exige `TEST_MIGRATE_DATABASE_URL` (fournie en CI) ; sauté sinon.

use kollega_core::Cents;
use kollega_policy::{ToolCallRequest, ToolRule};
use kollega_runtime::machine::{ExecutionPermit, ModelProvider, PlannedAction, ToolRunner};
use sqlx::{Connection, PgConnection};
use std::process::Command;
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

struct OneTool;
impl ModelProvider for OneTool {
    fn plan(&self, iteration: u32) -> PlannedAction {
        if iteration == 0 {
            PlannedAction::UseTool {
                request: ToolCallRequest {
                    tool_name: "doc.read".to_owned(),
                    ..ToolCallRequest::default()
                },
                model_cost: Cents(10),
                tool_cost: Cents(5),
            }
        } else {
            PlannedAction::Conclude {
                model_cost: Cents(5),
                answer: "fini".to_owned(),
            }
        }
    }
}

struct Echo;
impl ToolRunner for Echo {
    fn run(&self, permit: &ExecutionPermit) -> String {
        format!("ok {}", permit.tool())
    }
}

fn run_verify(app_url: &str, org: Uuid) -> (bool, String, String) {
    let output = Command::new(env!("CARGO_BIN_EXE_kollega"))
        .args(["audit", "verify", "--org", &org.to_string()])
        .env("DATABASE_URL", app_url)
        .output()
        .expect("exécution du binaire kollega");
    (
        output.status.success(),
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

#[tokio::test]
async fn audit_verify_says_yes_on_a_sound_chain_and_no_on_a_broken_one() {
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

    let org = Uuid::from_u128(0xC1_11);
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

    let app_url = app_url_from(&migrate_url);
    let db = kollega_store::Db::connect(&app_url)
        .await
        .expect("connexion");
    let mut tx = db.org_transaction(org).await.expect("tx seed");
    sqlx::query("INSERT INTO organizations (id, name) VALUES ($1, 'Org Audit CLI')")
        .bind(org)
        .execute(&mut *tx)
        .await
        .expect("org");
    sqlx::query("INSERT INTO credits (org_id, balance_cents) VALUES ($1, 10000)")
        .bind(org)
        .execute(&mut *tx)
        .await
        .expect("crédit");
    tx.commit().await.expect("commit seed");

    // Une tâche réelle produit une chaîne réelle.
    let task = Uuid::new_v4();
    kollega_store::driver::create_task(&db, org, task, Cents(500), 4)
        .await
        .expect("création");
    let rules = vec![ToolRule {
        tool_name: "doc.read".to_owned(),
        allowed: true,
        requires_approval: false,
        amount: None,
        recipients: None,
        paths: None,
    }];
    kollega_store::driver::run_task_step(&db, org, task, &OneTool, &Echo, &rules, None)
        .await
        .expect("pas");

    // --- 1. Chaîne saine : la commande doit dire OUI ------------------------
    let (ok, stdout, stderr) = run_verify(&app_url, org);
    assert!(ok, "chaîne saine refusée — stdout={stdout} stderr={stderr}");
    assert!(
        stdout.contains("intègre"),
        "la sortie doit dire ce qu'elle a vérifié : {stdout}"
    );
    assert!(
        stdout.contains("cohérente"),
        "la séquence doit être rapportée aussi : {stdout}"
    );

    // --- 2. Chaîne corrompue : la commande doit dire NON --------------------
    // Un acteur réécrit après coup ce qu'un agent a fait. Seul le rôle de
    // migration peut le faire — le rôle applicatif, lui, n'a pas ce droit
    // (invariant 4) — mais un attaquant en écriture directe, si.
    sqlx::query(
        "UPDATE audit_chain SET actor = 'quelqu_un_d_autre' WHERE org_id = $1 AND height = 0",
    )
    .bind(org)
    .execute(&mut admin)
    .await
    .expect("altération");

    let (ok, stdout, stderr) = run_verify(&app_url, org);
    assert!(
        !ok,
        "une chaîne ALTÉRÉE doit faire échouer la commande — stdout={stdout}"
    );
    assert!(
        stderr.contains("ROMPUE"),
        "l'échec doit être nommé, pas muet : {stderr}"
    );
}
