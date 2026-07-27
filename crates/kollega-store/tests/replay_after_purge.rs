//! Après une purge RGPD, un rejeu REFUSE — il ne refait pas l'effet.
//!
//! La migration 0004 l'écrit noir sur blanc : « un rejeu dont le contenu a
//! été purgé échoue explicitement — il ne ré-exécute surtout pas ».
//! **Rien ne le vérifiait.** `StoreError::CorruptState` n'était produite
//! par aucun test, et c'est cette variante-là qui porte le refus.
//!
//! Pourquoi c'est sérieux. L'idempotence repose sur la mémoire des effets :
//! `tool_call_effects` retient QU'un appel a eu lieu, et `audit_content`
//! retient CE QU'il a rendu. La purge RGPD efface le second, jamais le
//! premier. Un rejeu se retrouve donc devant un effet dont le résultat a
//! disparu — et il n'a que deux conduites possibles :
//!
//! - refaire l'appel, donc envoyer un SECOND mail au client d'une
//!   organisation qui vient précisément d'exercer son droit à l'effacement ;
//! - refuser, en nommant la cause.
//!
//! Le code choisit la seconde. Ce test l'y tient. Sans lui, la première
//! serait une régression d'une ligne, invisible en revue : il suffirait de
//! traiter le contenu absent comme un effet inconnu.
//!
//! Exige `TEST_MIGRATE_DATABASE_URL` (fournie en CI) ; sauté sinon.

use kollega_core::Cents;
use kollega_policy::{Bound, ToolCallRequest, ToolRule};
use kollega_runtime::machine::{
    ApprovalDecision, ExecutionPermit, ModelProvider, PlannedAction, ToolRunner,
};
use kollega_store::{driver, StoreError};
use sqlx::{Connection, PgConnection, Row as _};
use std::sync::atomic::{AtomicUsize, Ordering};
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

/// Envoie un mail à l'itération 0, conclut ensuite.
struct EnvoieUnMail;

impl ModelProvider for EnvoieUnMail {
    fn plan(&self, iteration: u32) -> PlannedAction {
        if iteration == 0 {
            PlannedAction::UseTool {
                request: ToolCallRequest {
                    tool_name: "mail.send".to_owned(),
                    amount: Some(Cents(1_000)),
                    recipient_count: Some(1),
                    paths: Vec::new(),
                },
                model_cost: Cents(5),
                tool_cost: Cents(5),
            }
        } else {
            PlannedAction::Conclude {
                model_cost: Cents(5),
                answer: "relance envoyée".to_owned(),
            }
        }
    }
}

/// Compte les exécutions réelles : c'est le second mail qu'on traque.
struct MailCompte {
    envois: AtomicUsize,
}

impl ToolRunner for MailCompte {
    fn run(&self, permit: &ExecutionPermit) -> String {
        self.envois.fetch_add(1, Ordering::SeqCst);
        format!("envoyé à l'itération {}", permit.iteration())
    }
}

fn regles() -> Vec<ToolRule> {
    vec![ToolRule {
        tool_name: "mail.send".to_owned(),
        allowed: true,
        requires_approval: true,
        amount: Some(Bound::two_tier(Cents(50_000), Cents(500_000)).expect("bornes")),
        recipients: Some(Bound::two_tier(10u32, 100).expect("bornes")),
        paths: None,
    }]
}

#[tokio::test]
async fn a_replay_after_the_rgpd_purge_refuses_instead_of_resending() {
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

    let org = Uuid::from_u128(0x0009_9AC1);
    let mut admin = PgConnection::connect(&migrate_url).await.expect("admin");
    for table in [
        "tool_call_effects",
        "audit_chain",
        "audit_content",
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
    let mut tx = db.org_transaction(org).await.expect("tx");
    sqlx::query("INSERT INTO organizations (id, name) VALUES ($1, 'Org Purge')")
        .bind(org)
        .execute(&mut *tx)
        .await
        .expect("organisation");
    sqlx::query("INSERT INTO credits (org_id, balance_cents) VALUES ($1, 100000)")
        .bind(org)
        .execute(&mut *tx)
        .await
        .expect("crédit");
    tx.commit().await.expect("commit");

    let task = Uuid::from_u128(0x0009_9AC2);
    driver::create_task(&db, org, task, Cents(10_000), 8)
        .await
        .expect("tâche");
    let outils = MailCompte {
        envois: AtomicUsize::new(0),
    };

    // 1. Le pas s'arrête sur une demande de validation.
    driver::run_task_step(&db, org, task, &EnvoieUnMail, &outils, &regles(), None)
        .await
        .expect("premier pas");

    // On capture l'état SUSPENDU : c'est lui qu'on remettra pour rejouer.
    let suspendu: String = sqlx::query("SELECT state::text FROM tasks WHERE id = $1")
        .bind(task)
        .fetch_one(&mut admin)
        .await
        .expect("lecture de l'état suspendu")
        .get(0);

    // 2. Validation : le mail PART pour de vrai.
    driver::run_task_step(
        &db,
        org,
        task,
        &EnvoieUnMail,
        &outils,
        &regles(),
        Some(ApprovalDecision::Approve),
    )
    .await
    .expect("pas validé");
    assert_eq!(
        outils.envois.load(Ordering::SeqCst),
        1,
        "le mail doit être parti une fois"
    );

    // 3. L'organisation exerce son droit à l'effacement : le CONTENU part,
    //    la trace de l'effet reste (elle ne contient aucune donnée client).
    let purges = driver::purge_org_content(&db, org)
        .await
        .expect("purge RGPD");
    assert!(purges > 0, "la purge doit avoir retiré du contenu");

    // 4. Retour arrière de l'état seul, comme après une panne, puis rejeu.
    sqlx::query("UPDATE tasks SET state = $2::jsonb, status = 'waiting_approval' WHERE id = $1")
        .bind(task)
        .bind(&suspendu)
        .execute(&mut admin)
        .await
        .expect("retour arrière de l'état");

    let refus = driver::run_task_step(
        &db,
        org,
        task,
        &EnvoieUnMail,
        &outils,
        &regles(),
        Some(ApprovalDecision::Approve),
    )
    .await
    .expect_err("le rejeu doit REFUSER : le résultat de l'effet a été purgé");

    match &refus {
        StoreError::CorruptState(message) => assert!(
            message.contains("purg"),
            "le refus doit NOMMER la cause — un message générique enverrait \
             chercher une corruption là où il n'y a qu'une purge légitime : {message}"
        ),
        autre => panic!("attendu CorruptState, reçu : {autre}"),
    }

    // LE POINT : aucun second mail. Refuser, c'est refuser d'agir.
    assert_eq!(
        outils.envois.load(Ordering::SeqCst),
        1,
        "le rejeu ne doit surtout pas renvoyer le mail — l'organisation \
         vient d'exercer son droit à l'effacement"
    );
}
