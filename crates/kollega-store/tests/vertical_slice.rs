//! LA TRANCHE VERTICALE (session du 28/07, bloc 1).
//!
//! Une tâche est créée, soumise à la politique, exécutée, débitée du crédit,
//! journalisée dans PostgreSQL (attestation et contenu SÉPARÉS), interrompue,
//! reprise depuis la base, et terminée avec le même résultat qu'un parcours
//! direct. Rien de moins ne compte.
//!
//! Exige `TEST_MIGRATE_DATABASE_URL` (fournie en CI) ; sauté sinon, avec un
//! message explicite.

use kollega_core::{Cents, TaskStatus};
use kollega_policy::ToolRule;
use kollega_runtime::machine::{ApprovalDecision, ModelProvider, PlannedAction, ToolRunner};
use kollega_store::driver;
use sqlx::{Connection, PgConnection, Row as _};
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

/// Modèle scripté : rejoue une action par itération, déterministe.
struct ScriptedModel {
    actions: Vec<PlannedAction>,
}

impl ModelProvider for ScriptedModel {
    fn plan(&self, iteration: u32) -> PlannedAction {
        self.actions[iteration as usize].clone()
    }
}

/// Exécuteur d'outils factice qui COMPTE ses exécutions réelles : c'est ce
/// compteur qui prouve qu'un effet n'a pas été accompli deux fois.
#[derive(Default)]
struct FixedTools {
    executions: std::sync::atomic::AtomicUsize,
}

impl FixedTools {
    fn count(&self) -> usize {
        self.executions.load(std::sync::atomic::Ordering::SeqCst)
    }
}

impl ToolRunner for FixedTools {
    fn run(&self, tool: &str, iteration: u32) -> String {
        self.executions
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        format!("exécuté : {tool} (itération {iteration})")
    }
}

fn relance_rules() -> Vec<ToolRule> {
    vec![ToolRule {
        tool_name: "mail.send".to_owned(),
        allowed: true,
        requires_approval: true,
        amount: None,
        recipients: None,
        paths: None,
    }]
}

fn relance_script() -> ScriptedModel {
    ScriptedModel {
        actions: vec![
            PlannedAction::UseTool {
                tool: "mail.send".to_owned(),
                model_cost: Cents(30),
                tool_cost: Cents(20),
            },
            PlannedAction::Conclude {
                model_cost: Cents(5),
                answer: "relance envoyée".to_owned(),
            },
        ],
    }
}

async fn seed_org(db: &kollega_store::Db, org: Uuid, name: &str, balance: i64) {
    let mut tx = db.org_transaction(org).await.expect("transaction org");
    sqlx::query("INSERT INTO organizations (id, name) VALUES ($1, $2)")
        .bind(org)
        .bind(name)
        .execute(&mut *tx)
        .await
        .expect("insertion organisation");
    sqlx::query("INSERT INTO credits (org_id, balance_cents) VALUES ($1, $2)")
        .bind(org)
        .bind(balance)
        .execute(&mut *tx)
        .await
        .expect("insertion crédit");
    tx.commit().await.expect("commit seed");
}

#[tokio::test]
async fn the_vertical_slice_goes_through() {
    let Some(migrate_url) = migrate_url() else {
        eprintln!(
            "IGNORÉ : TEST_MIGRATE_DATABASE_URL absent — la tranche verticale \
             exige une base réelle (exécutée en CI)."
        );
        return;
    };

    kollega_store::run_migrations(&migrate_url)
        .await
        .expect("migrations");
    kollega_store::set_app_role_password(&migrate_url, APP_PASSWORD)
        .await
        .expect("mot de passe kollega_app");

    // Organisations dédiées à ce test : aucune collision avec les autres.
    let org = Uuid::from_u128(0x51_1CE);
    let witness = Uuid::from_u128(0x51_1CF);
    let db = kollega_store::Db::connect(&app_url_from(&migrate_url))
        .await
        .expect("connexion kollega_app");

    // Nettoyage rejouable : ce test peut retourner sur une base déjà peuplée.
    let mut admin = PgConnection::connect(&migrate_url).await.expect("admin");
    for o in [org, witness] {
        sqlx::query("DELETE FROM audit_content WHERE org_id = $1")
            .bind(o)
            .execute(&mut admin)
            .await
            .expect("purge contenu");
        sqlx::query("DELETE FROM audit_chain WHERE org_id = $1")
            .bind(o)
            .execute(&mut admin)
            .await
            .expect("purge chaîne (rôle migration : le rôle applicatif, lui, ne PEUT pas)");
        sqlx::query("DELETE FROM tasks WHERE org_id = $1")
            .bind(o)
            .execute(&mut admin)
            .await
            .expect("purge tâches");
        sqlx::query("DELETE FROM credits WHERE org_id = $1")
            .bind(o)
            .execute(&mut admin)
            .await
            .expect("purge crédits");
        sqlx::query("DELETE FROM users WHERE org_id = $1")
            .bind(o)
            .execute(&mut admin)
            .await
            .expect("purge users");
        sqlx::query("DELETE FROM organizations WHERE id = $1")
            .bind(o)
            .execute(&mut admin)
            .await
            .expect("purge org");
    }
    seed_org(&db, org, "Org Tranche", 10_000).await;
    seed_org(&db, witness, "Org Témoin", 10_000).await;

    let tools = FixedTools::default();

    // ---- 1. Créée → politique → suspendue (validation humaine) -------------
    let t1 = Uuid::new_v4();
    driver::create_task(&db, org, t1, Cents(500), 8)
        .await
        .expect("création de tâche");
    let step = driver::run_task_step(
        &db,
        org,
        t1,
        &relance_script(),
        &tools,
        &relance_rules(),
        None,
    )
    .await
    .expect("premier pas");
    assert_eq!(step.status, TaskStatus::WaitingApproval);
    assert_eq!(
        step.org_balance,
        Cents(10_000),
        "rien n'est facturé avant validation"
    );

    // ---- 2. INTERRUPTION : le « processus » redémarre ----------------------
    drop(db);
    let db = kollega_store::Db::connect(&app_url_from(&migrate_url))
        .await
        .expect("reconnexion après interruption");

    // ---- 3. Reprise DEPUIS LA BASE, approbation, exécution, débit ----------
    let step = driver::run_task_step(
        &db,
        org,
        t1,
        &relance_script(),
        &tools,
        &relance_rules(),
        Some(ApprovalDecision::Approve),
    )
    .await
    .expect("reprise après interruption");
    assert_eq!(step.status, TaskStatus::Succeeded);
    assert_eq!(step.conclusion.as_deref(), Some("relance envoyée"));
    assert_eq!(
        step.org_balance,
        Cents(10_000 - 55),
        "30+20 approuvés, puis 5 de conclusion"
    );

    // ---- 4. Même résultat qu'un parcours direct (sans redémarrage) ---------
    let t2 = Uuid::new_v4();
    driver::create_task(&db, org, t2, Cents(500), 8)
        .await
        .expect("création T2");
    driver::run_task_step(
        &db,
        org,
        t2,
        &relance_script(),
        &tools,
        &relance_rules(),
        None,
    )
    .await
    .expect("T2 pas 1");
    let direct = driver::run_task_step(
        &db,
        org,
        t2,
        &relance_script(),
        &tools,
        &relance_rules(),
        Some(ApprovalDecision::Approve),
    )
    .await
    .expect("T2 pas 2");
    assert_eq!(direct.status, TaskStatus::Succeeded);
    assert_eq!(direct.conclusion.as_deref(), Some("relance envoyée"));
    assert_eq!(direct.org_balance, Cents(10_000 - 110));

    // Les deux tâches ont émis la même séquence d'événements.
    let mut tx = db.org_transaction(org).await.expect("tx lecture");
    for task in [t1, t2] {
        let actions: Vec<String> =
            sqlx::query("SELECT action FROM audit_chain WHERE actor = $1 ORDER BY height")
                .bind(task.to_string())
                .fetch_all(&mut *tx)
                .await
                .expect("lecture chaîne")
                .into_iter()
                .map(|r| r.get(0))
                .collect();
        assert_eq!(
            actions,
            vec![
                "task_started",
                "tool_call_intended",
                "approval_requested",
                "approval_resolved",
                "tool_call_completed",
                "task_finished",
            ],
            "séquence d'attestations de {task}"
        );
    }
    drop(tx);

    // ---- 5. La chaîne se vérifie de bout en bout ---------------------------
    let check = driver::verify_org_chain(&db, org)
        .await
        .expect("vérification");
    assert_eq!(check.entries, 12, "6 attestations par tâche");

    // ---- 6. Fourche impossible : PRIMARY KEY (org_id, height) -------------
    // Deux écrivains visent la même hauteur : le second échoue en 23505.
    let mut tx_a = db.org_transaction(org).await.expect("écrivain A");
    let mut tx_b = db.org_transaction(org).await.expect("écrivain B");
    let next: i64 = sqlx::query("SELECT COALESCE(MAX(height), -1) + 1 FROM audit_chain")
        .fetch_one(&mut *tx_a)
        .await
        .expect("hauteur")
        .get(0);
    let fake = vec![0u8; 32];
    let insert = "INSERT INTO audit_chain \
                  (org_id, height, prev_hash, entry_hash, actor, action, timestamp_micros) \
                  VALUES ($1, $2, NULL, $3, 'test', 'fork_probe', 0)";
    sqlx::query(insert)
        .bind(org)
        .bind(next)
        .bind(&fake)
        .execute(&mut *tx_a)
        .await
        .expect("écrivain A insère");
    tx_a.commit().await.expect("commit A");
    let clash = sqlx::query(insert)
        .bind(org)
        .bind(next)
        .bind(&fake)
        .execute(&mut *tx_b)
        .await;
    match clash {
        Err(sqlx::Error::Database(db_err)) => {
            assert_eq!(
                db_err.code().as_deref(),
                Some("23505"),
                "attendu une violation d'unicité"
            );
        }
        other => panic!("la fourche aurait dû violer l'unicité : {other:?}"),
    }
    drop(tx_b);
    // La sonde de fourche a écrit une entrée NON chaînée (hachage factice) :
    // la vérification doit maintenant ÉCHOUER — preuve qu'elle voit tout.
    assert!(matches!(
        driver::verify_org_chain(&db, org).await,
        Err(kollega_store::StoreError::ChainBroken(_))
    ));
    // Retrait de la sonde par le rôle de migration (le rôle applicatif ne
    // peut pas : GRANT sans DELETE — c'est le test de l'ajout seul).
    let mut tx = db.org_transaction(org).await.expect("tx app");
    let denied = sqlx::query("DELETE FROM audit_chain WHERE action = 'fork_probe'")
        .execute(&mut *tx)
        .await;
    assert!(
        denied.is_err(),
        "kollega_app ne doit pas pouvoir supprimer dans la chaîne"
    );
    drop(tx);
    sqlx::query("DELETE FROM audit_chain WHERE org_id = $1 AND action = 'fork_probe'")
        .bind(org)
        .execute(&mut admin)
        .await
        .expect("retrait de la sonde (rôle migration)");
    driver::verify_org_chain(&db, org)
        .await
        .expect("chaîne redevenue saine");

    // ---- 6 bis. IDEMPOTENCE : l'effet a eu lieu, l'état revient en arrière -
    // LE scénario dangereux du chemin réel : le mail est parti, puis la
    // transaction du pas est annulée (conflit, panne, rejeu). Sans mémoire
    // des effets, le rejeu envoie un SECOND mail. Ici, on le reconstitue
    // exactement : on capture l'état suspendu, on laisse le pas s'exécuter
    // (l'effet a lieu), on REMET l'état d'avant, et on rejoue.
    let t3 = Uuid::new_v4();
    driver::create_task(&db, org, t3, Cents(500), 8)
        .await
        .expect("création T3");
    driver::run_task_step(
        &db,
        org,
        t3,
        &relance_script(),
        &tools,
        &relance_rules(),
        None,
    )
    .await
    .expect("T3 suspendue");
    // L'état suspendu, tel qu'il est en base avant l'exécution de l'outil.
    let suspended_state: String = sqlx::query("SELECT state::text FROM tasks WHERE id = $1")
        .bind(t3)
        .fetch_one(&mut admin)
        .await
        .expect("capture de l'état suspendu")
        .get(0);

    let before = tools.count();
    driver::run_task_step(
        &db,
        org,
        t3,
        &relance_script(),
        &tools,
        &relance_rules(),
        Some(ApprovalDecision::Approve),
    )
    .await
    .expect("T3 exécute l'outil");
    assert_eq!(tools.count(), before + 1, "l'outil s'exécute une fois");

    // Retour arrière de l'ÉTAT SEUL (le rôle de migration écrit ce que le
    // rôle applicatif ne peut pas défaire) : l'effet, lui, reste enregistré.
    sqlx::query("UPDATE tasks SET state = $2::jsonb, status = 'waiting_approval' WHERE id = $1")
        .bind(t3)
        .bind(&suspended_state)
        .execute(&mut admin)
        .await
        .expect("remise de l'état suspendu");

    let replay = driver::run_task_step(
        &db,
        org,
        t3,
        &relance_script(),
        &tools,
        &relance_rules(),
        Some(ApprovalDecision::Approve),
    )
    .await
    .expect("rejeu du pas");
    assert_eq!(
        tools.count(),
        before + 1,
        "LE POINT : le rejeu ne doit PAS ré-exécuter l'outil — pas de second mail"
    );
    assert_eq!(
        replay.status,
        TaskStatus::Succeeded,
        "le rejeu aboutit quand même"
    );

    // L'effet est enregistré une seule fois, sous une identité DÉRIVÉE
    // (donc identique d'un rejeu à l'autre).
    let mut tx = db.org_transaction(org).await.expect("tx effets");
    let effects: i64 = sqlx::query("SELECT count(*) FROM tool_call_effects WHERE task_id = $1")
        .bind(t3)
        .fetch_one(&mut *tx)
        .await
        .expect("comptage des effets")
        .get(0);
    assert_eq!(
        effects, 1,
        "un seul effet enregistré pour un seul effet réel"
    );
    drop(tx);

    // Le rôle applicatif ne peut ni défaire ni réécrire un effet.
    let mut tx = db.org_transaction(org).await.expect("tx effets 2");
    assert!(
        sqlx::query("DELETE FROM tool_call_effects")
            .execute(&mut *tx)
            .await
            .is_err(),
        "un effet realisé ne se supprime pas"
    );
    drop(tx);

    // ---- 7. Isolation : l'organisation témoin ne voit RIEN -----------------
    let mut tx = db.org_transaction(witness).await.expect("tx témoin");
    let seen: i64 = sqlx::query("SELECT count(*) FROM audit_chain")
        .fetch_one(&mut *tx)
        .await
        .expect("comptage témoin")
        .get(0);
    assert_eq!(seen, 0, "la chaîne de l'org ne fuit pas vers le témoin");
    drop(tx);

    // ---- 8. Purge RGPD du contenu : la chaîne se vérifie TOUJOURS ----------
    let mut tx = db.org_transaction(org).await.expect("tx contenu");
    let contents_before: i64 = sqlx::query("SELECT count(*) FROM audit_content")
        .fetch_one(&mut *tx)
        .await
        .expect("comptage contenu")
        .get(0);
    drop(tx);
    assert!(
        contents_before >= 6,
        "les contenus d'audit existent avant purge"
    );
    let entries_before = driver::verify_org_chain(&db, org)
        .await
        .expect("chaîne saine avant purge")
        .entries;
    let purged = driver::purge_org_content(&db, org).await.expect("purge");
    assert_eq!(purged, contents_before as u64);
    let check = driver::verify_org_chain(&db, org)
        .await
        .expect("la chaîne d'attestations survit à la purge du contenu");
    assert_eq!(
        check.entries,
        entries_before + 1,
        "la chaîne ne perd rien à la purge, et gagne la trace de purge"
    );
    let mut tx = db.org_transaction(org).await.expect("tx contenu après");
    let contents_after: i64 = sqlx::query("SELECT count(*) FROM audit_content")
        .fetch_one(&mut *tx)
        .await
        .expect("comptage après purge")
        .get(0);
    assert_eq!(
        contents_after, 1,
        "seul le contenu de la trace de purge demeure"
    );
}
