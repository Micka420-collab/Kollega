//! Une tâche d'une AUTRE organisation est indiscernable d'une tâche
//! inexistante.
//!
//! Trou trouvé le 29/07 en cherchant les variantes d'erreur qu'aucun test
//! ne produit : `StoreError::TaskNotFound` n'apparaissait qu'à deux
//! endroits du dépôt — sa définition et son unique production. **Aucun
//! test ne l'obtenait.**
//!
//! Ce n'est pas qu'une question de couverture. Le message de cette
//! variante dit « tâche introuvable dans ce contexte d'organisation », et
//! cette formulation porte une propriété de sécurité : sous RLS, la tâche
//! d'une autre organisation est INVISIBLE, donc la même erreur sort dans
//! les deux cas. Un appelant ne peut pas distinguer « n'existe pas » de
//! « appartient à quelqu'un d'autre » — c'est ce qui empêche d'énumérer
//! les tâches d'autrui en observant les réponses.
//!
//! Si un jour quelqu'un « améliorait » le diagnostic en distinguant les
//! deux cas, il ouvrirait un canal d'énumération sans toucher à la RLS ni
//! à aucune politique. Ce test refuse cette amélioration-là.
//!
//! Exige `TEST_MIGRATE_DATABASE_URL` (fournie en CI) ; sauté sinon.

use kollega_core::Cents;
use kollega_policy::ToolRule;
use kollega_runtime::machine::{ExecutionPermit, ModelProvider, PlannedAction, ToolRunner};
use kollega_store::{driver, StoreError};
use sqlx::{Connection, PgConnection};
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

struct Conclut;

impl ModelProvider for Conclut {
    fn plan(&self, _iteration: u32) -> PlannedAction {
        PlannedAction::Conclude {
            model_cost: Cents(1),
            answer: "terminé".to_owned(),
        }
    }
}

struct NoTools;

impl ToolRunner for NoTools {
    fn run(&self, _permit: &ExecutionPermit) -> String {
        unreachable!("ce scénario n'appelle aucun outil")
    }
}

#[tokio::test]
async fn a_task_of_another_org_is_indistinguishable_from_a_missing_one() {
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

    let org_a = Uuid::from_u128(0x0007_A55A);
    let org_b = Uuid::from_u128(0x0007_A55B);
    let mut admin = PgConnection::connect(&migrate_url).await.expect("admin");
    for org in [org_a, org_b] {
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
    }

    let db = kollega_store::Db::connect(&app_url_from(&migrate_url))
        .await
        .expect("connexion");

    for (org, nom) in [(org_a, "Org A"), (org_b, "Org B")] {
        let mut tx = db.org_transaction(org).await.expect("tx");
        sqlx::query("INSERT INTO organizations (id, name) VALUES ($1, $2)")
            .bind(org)
            .bind(nom)
            .execute(&mut *tx)
            .await
            .expect("organisation");
        sqlx::query("INSERT INTO credits (org_id, balance_cents) VALUES ($1, 100000)")
            .bind(org)
            .execute(&mut *tx)
            .await
            .expect("crédit");
        tx.commit().await.expect("commit");
    }

    // Une tâche BIEN RÉELLE, dans l'organisation A.
    let task_de_a = Uuid::from_u128(0x0007_A55C);
    driver::create_task(&db, org_a, task_de_a, Cents(10_000), 4)
        .await
        .expect("tâche de A");

    let rules: Vec<ToolRule> = Vec::new();

    // 1. Une tâche qui n'existe nulle part.
    let inconnue = driver::run_task_step(
        &db,
        org_a,
        Uuid::from_u128(0x0007_DEAD),
        &Conclut,
        &NoTools,
        &rules,
        None,
    )
    .await
    .expect_err("une tâche inexistante ne peut pas avancer");
    assert!(
        matches!(inconnue, StoreError::TaskNotFound),
        "attendu TaskNotFound, reçu : {inconnue}"
    );

    // 2. LE POINT : la tâche de A, demandée depuis le contexte de B. Elle
    //    existe, mais la RLS la rend invisible — et l'erreur doit être la
    //    MÊME que pour une tâche inexistante.
    let volee = driver::run_task_step(&db, org_b, task_de_a, &Conclut, &NoTools, &rules, None)
        .await
        .expect_err("la tâche d'une autre organisation ne doit pas avancer");
    assert!(
        matches!(volee, StoreError::TaskNotFound),
        "la tâche d'une AUTRE organisation doit rendre exactement \
         TaskNotFound — toute erreur distincte dirait à B que cette tâche \
         existe, et permettrait d'énumérer celles d'autrui sans jamais \
         toucher à la RLS. Reçu : {volee}"
    );

    // Et la tâche de A n'a pas bougé : la tentative de B n'a rien fait.
    let pas = driver::run_task_step(&db, org_a, task_de_a, &Conclut, &NoTools, &rules, None)
        .await
        .expect("A peut faire avancer sa propre tâche");
    assert_eq!(
        pas.status,
        kollega_core::TaskStatus::Succeeded,
        "la tentative de B ne doit pas avoir altéré la tâche de A"
    );
}
