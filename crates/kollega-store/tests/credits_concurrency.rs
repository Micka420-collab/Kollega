//! Invariant 5 sous CONCURRENCE RÉELLE — le test qui manquait.
//!
//! La règle comptable pure était prouvée depuis longtemps ; ce qui ne
//! l'était pas, c'est qu'elle tienne quand DEUX tâches de la même
//! organisation avancent en même temps. C'est pourtant là que le découvert
//! se produit : chaque tâche porte un instantané du solde dans son état
//! sérialisé, et deux instantanés du même solde autorisent deux fois la
//! même dépense.
//!
//! Ce que ce test prouve : le solde est LU ET VERROUILLÉ dans la
//! transaction du pas (`FOR UPDATE`) puis rechargé dans le budget
//! (`Budget::refreshed`) — donc la seconde tâche voit le solde APRÈS la
//! première, et un client ne consomme jamais à découvert.
//!
//! Exige `TEST_MIGRATE_DATABASE_URL` (fournie en CI) ; sauté sinon.

use kollega_core::{Cents, TaskStatus};
use kollega_policy::ToolRule;
use kollega_runtime::machine::{ModelProvider, PlannedAction, ToolRunner};
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

/// Modèle qui conclut immédiatement, à un coût donné.
struct CostlyConclusion {
    cost: Cents,
}

impl ModelProvider for CostlyConclusion {
    fn plan(&self, _iteration: u32) -> PlannedAction {
        PlannedAction::Conclude {
            model_cost: self.cost,
            answer: "terminé".to_owned(),
        }
    }
}

struct NoTools;

impl ToolRunner for NoTools {
    fn run(&self, _tool: &str, _iteration: u32) -> String {
        unreachable!("ce scénario n'appelle aucun outil")
    }
}

#[tokio::test]
async fn two_concurrent_tasks_never_overdraw_the_credit() {
    let Ok(migrate_url) = std::env::var("TEST_MIGRATE_DATABASE_URL") else {
        eprintln!(
            "IGNORÉ : TEST_MIGRATE_DATABASE_URL absent — la concurrence du \
             crédit exige une base réelle (exécutée en CI)."
        );
        return;
    };

    kollega_store::run_migrations(&migrate_url)
        .await
        .expect("migrations");
    kollega_store::set_app_role_password(&migrate_url, APP_PASSWORD)
        .await
        .expect("mot de passe kollega_app");

    // Organisation dédiée : ce test ne partage rien avec les autres.
    let org = Uuid::from_u128(0xC0_1CE);
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

    // Solde de 100 : de quoi payer UNE seule des deux tâches à 60.
    let mut tx = db.org_transaction(org).await.expect("tx seed");
    sqlx::query("INSERT INTO organizations (id, name) VALUES ($1, 'Org Concurrence')")
        .bind(org)
        .execute(&mut *tx)
        .await
        .expect("org");
    sqlx::query("INSERT INTO credits (org_id, balance_cents) VALUES ($1, 100)")
        .bind(org)
        .execute(&mut *tx)
        .await
        .expect("crédit");
    tx.commit().await.expect("commit seed");

    let (a, b) = (Uuid::new_v4(), Uuid::new_v4());
    // Les DEUX tâches sont créées AVANT tout pas : chacune emporte donc un
    // instantané du solde à 100. C'est exactement le piège — sans
    // rechargement, chacune se croirait autorisée à dépenser 60.
    for task in [a, b] {
        driver::create_task(&db, org, task, Cents(1_000), 4)
            .await
            .expect("création");
    }

    let model = CostlyConclusion { cost: Cents(60) };
    let rules: Vec<ToolRule> = Vec::new();
    let (ra, rb) = tokio::join!(
        driver::run_task_step(&db, org, a, &model, &NoTools, &rules, None),
        driver::run_task_step(&db, org, b, &model, &NoTools, &rules, None),
    );
    let ra = ra.expect("pas A");
    let rb = rb.expect("pas B");

    // Exactement UNE tâche aboutit ; l'autre s'arrête faute de crédit.
    let succeeded = [&ra, &rb]
        .iter()
        .filter(|r| r.status == TaskStatus::Succeeded)
        .count();
    assert_eq!(
        succeeded, 1,
        "une seule tâche doit passer : A={:?} B={:?}",
        ra.status, rb.status
    );

    // LE POINT : le solde n'est jamais passé sous zéro, et il vaut
    // exactement ce qu'une seule dépense laisse.
    let mut tx = db.org_transaction(org).await.expect("tx lecture");
    let balance: i64 = sqlx::query("SELECT balance_cents FROM credits WHERE org_id = $1")
        .bind(org)
        .fetch_one(&mut *tx)
        .await
        .expect("lecture du solde")
        .get(0);
    assert_eq!(
        balance, 40,
        "100 − 60 : la seconde tâche n'a rien pu débiter"
    );
    assert!(balance >= 0, "un client ne consomme jamais à découvert");
}

/// Invariant 6 sur base réelle : le plafond de tâche arrête PROPREMENT.
///
/// Le noyau pur le prouvait déjà ; ce qui manquait, c'est que l'arrêt
/// survive au passage par la base — statut distinct persisté, et surtout
/// RIEN de facturé. « Jamais de dégradation silencieuse » ne vaut que si le
/// dirigeant peut lire, après coup, que la tâche s'est arrêtée au plafond
/// et non qu'elle a échoué.
#[tokio::test]
async fn the_cost_ceiling_stops_the_task_cleanly_and_bills_nothing() {
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

    let org = Uuid::from_u128(0xCE_111);
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
    sqlx::query("INSERT INTO organizations (id, name) VALUES ($1, 'Org Plafond')")
        .bind(org)
        .execute(&mut *tx)
        .await
        .expect("org");
    // Crédit LARGE : ce n'est pas l'argent qui manque, c'est le plafond de
    // la tâche qui borne — les deux protections sont bien distinctes.
    sqlx::query("INSERT INTO credits (org_id, balance_cents) VALUES ($1, 100000)")
        .bind(org)
        .execute(&mut *tx)
        .await
        .expect("crédit");
    tx.commit().await.expect("commit seed");

    let task = Uuid::new_v4();
    driver::create_task(&db, org, task, Cents(40), 4)
        .await
        .expect("création");
    let step = driver::run_task_step(
        &db,
        org,
        task,
        &CostlyConclusion { cost: Cents(60) }, // 60 > plafond 40
        &NoTools,
        &Vec::new(),
        None,
    )
    .await
    .expect("le pas aboutit — l'arrêt au plafond n'est pas une erreur");

    assert_eq!(
        step.status,
        TaskStatus::AbortedCostCeiling,
        "arrêt au plafond, DISTINCT d'un échec"
    );
    assert_eq!(
        step.org_balance,
        Cents(100_000),
        "un appel refusé au plafond n'est pas facturé"
    );

    // Le statut est LISIBLE en base : c'est ce que le dirigeant verra.
    let mut tx = db.org_transaction(org).await.expect("tx lecture");
    let status: String = sqlx::query("SELECT status FROM tasks WHERE id = $1")
        .bind(task)
        .fetch_one(&mut *tx)
        .await
        .expect("lecture du statut")
        .get(0);
    assert_eq!(
        status, "aborted_cost_ceiling",
        "le statut persisté nomme le plafond, il ne dit pas « échec »"
    );
}
