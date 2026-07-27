//! ÉTAPE 5 — LA TRANCHE TRAVERSE LE BINAIRE.
//!
//! La tranche verticale existait déjà, mais au niveau du PILOTE : un test
//! appelait `driver::run_task_step` en bibliothèque. Personne n'avait jamais
//! fait passer une tâche par le serveur, et c'est le serveur que l'image
//! publiée démarre. Entre les deux, il y avait une différence qu'aucun test
//! pur ne pouvait voir : le futur du pilote n'était pas `Send`, donc `axum`
//! refusait le handler à la compilation — la boucle n'était appelable que
//! depuis un test mono-tâche.
//!
//! Ici, une tâche est **créée, soumise à la politique, exécutée, débitée,
//! journalisée en base, interrompue, reprise depuis la base, terminée avec le
//! même résultat qu'un parcours direct** — le tout en parlant HTTP à un vrai
//! serveur sur une vraie socket.
//!
//! L'INTERRUPTION est réelle et plus dure que celle du test de pilote : le
//! premier serveur est ARRÊTÉ (arrêt propre, pool fermé, état en mémoire
//! perdu), et un SECOND serveur — nouveau pool, nouveau routeur, nouvel agent
//! — reprend la tâche. Rien ne passe de l'un à l'autre hormis PostgreSQL.
//!
//! Exige `TEST_MIGRATE_DATABASE_URL` (fournie en CI) ; sauté sinon.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use kollega_api::Agent;
use kollega_core::Cents;
use kollega_policy::{Bound, ToolCallRequest, ToolRule};
use kollega_runtime::machine::{ExecutionPermit, ModelProvider, PlannedAction, ToolRunner};
use sqlx::{Connection as _, Row as _};
use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};
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

/// Vrai si la réponse porte ce CODE de statut.
///
/// Le code, jamais la phrase qui le suit : elle est indicative dans HTTP/1.1
/// et un intermédiaire peut la réécrire (leçon d'un rouge de CI).
fn a_le_statut(reponse: &str, code: u16) -> bool {
    reponse.starts_with(&format!("HTTP/1.1 {code} "))
}

/// Le corps de la réponse, après la ligne vide qui clôt les en-têtes.
fn corps(reponse: &str) -> &str {
    reponse.split_once("\r\n\r\n").map_or("", |(_, c)| c)
}

async fn requete(addr: std::net::SocketAddr, brut: &str) -> String {
    let mut socket = tokio::net::TcpStream::connect(addr)
        .await
        .expect("connexion au serveur");
    socket
        .write_all(brut.as_bytes())
        .await
        .expect("envoi de la requête");
    let mut reponse = Vec::new();
    socket
        .read_to_end(&mut reponse)
        .await
        .expect("lecture de la réponse");
    String::from_utf8_lossy(&reponse).into_owned()
}

async fn poste(addr: std::net::SocketAddr, chemin: &str, json: &str) -> String {
    requete(
        addr,
        &format!(
            "POST {chemin} HTTP/1.1\r\nHost: k\r\nContent-Type: application/json\r\n\
             Content-Length: {}\r\nConnection: close\r\n\r\n{json}",
            json.len()
        ),
    )
    .await
}

/// Modèle scripté : une action par itération, déterministe.
///
/// Ce n'est PAS un modèle de démonstration câblé dans le produit — le binaire
/// démarre sans agent. C'est la pièce que le test branche au bout du même
/// port que branchera le vrai `ModelProvider`.
struct ScriptedModel {
    actions: Vec<PlannedAction>,
}

impl ModelProvider for ScriptedModel {
    fn plan(&self, iteration: u32) -> PlannedAction {
        self.actions[iteration as usize].clone()
    }
}

/// Exécuteur qui COMPTE ses exécutions réelles : c'est ce compteur qui dit si
/// un mail est parti deux fois. Il survit au redémarrage du serveur, comme le
/// monde extérieur survit au redéploiement.
#[derive(Default)]
struct CountingTools {
    executions: AtomicUsize,
}

impl CountingTools {
    fn count(&self) -> usize {
        self.executions.load(Ordering::SeqCst)
    }
}

impl ToolRunner for CountingTools {
    fn run(&self, permit: &ExecutionPermit) -> String {
        self.executions.fetch_add(1, Ordering::SeqCst);
        format!(
            "exécuté : {} (itération {})",
            permit.tool(),
            permit.iteration()
        )
    }
}

fn relance_rules() -> Vec<ToolRule> {
    vec![ToolRule {
        tool_name: "mail.send".to_owned(),
        allowed: true,
        requires_approval: true,
        amount: Some(Bound::two_tier(Cents(50_000), Cents(500_000)).expect("bornes valides")),
        recipients: Some(Bound::two_tier(10u32, 100).expect("bornes valides")),
        paths: None,
    }]
}

fn relance_script() -> ScriptedModel {
    ScriptedModel {
        actions: vec![
            PlannedAction::UseTool {
                request: ToolCallRequest {
                    tool_name: "mail.send".to_owned(),
                    recipient_count: Some(1),
                    amount: Some(Cents(12_500)),
                    paths: Vec::new(),
                },
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

/// Un serveur en vol : son adresse, et de quoi l'arrêter proprement.
struct Serveur {
    addr: std::net::SocketAddr,
    arret: tokio::sync::oneshot::Sender<()>,
    tache: tokio::task::JoinHandle<std::io::Result<()>>,
}

impl Serveur {
    /// Démarre le VRAI routeur du produit sur une socket éphémère.
    async fn demarrer(db: kollega_store::Db, agent: Option<Agent>) -> Serveur {
        let app = kollega_api::router(db, agent);
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("écoute");
        let addr = listener.local_addr().expect("adresse locale");
        let (arret, attendre) = tokio::sync::oneshot::channel::<()>();
        let tache = tokio::spawn(async move {
            kollega_api::serve_until(listener, app, async {
                attendre.await.ok();
            })
            .await
        });
        Serveur { addr, arret, tache }
    }

    /// Arrêt propre, et attente effective de la fin : à partir d'ici, plus
    /// rien de ce serveur n'est en mémoire.
    async fn arreter(self) {
        self.arret.send(()).expect("demande d'arrêt");
        tokio::time::timeout(Duration::from_secs(5), self.tache)
            .await
            .expect("le serveur doit rendre la main")
            .expect("la tâche du serveur ne doit pas paniquer")
            .expect("arrêt demandé, pas une panne");
    }
}

#[tokio::test]
async fn the_vertical_slice_goes_through_the_http_server_across_a_restart() {
    let Ok(migrate_url) = std::env::var("TEST_MIGRATE_DATABASE_URL") else {
        eprintln!(
            "IGNORÉ : TEST_MIGRATE_DATABASE_URL absent — la tranche par HTTP \
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

    // Organisations dédiées à CE test : aucune collision avec les autres
    // binaires de test, qui tournent en parallèle.
    let org = Uuid::from_u128(0x00_5E_11_01);
    let voisine = Uuid::from_u128(0x00_5E_11_02);

    let mut admin = sqlx::PgConnection::connect(&migrate_url)
        .await
        .expect("admin");
    for o in [org, voisine] {
        for table in [
            "tool_call_effects",
            "audit_chain",
            "audit_content",
            "tasks",
            "credits",
            "users",
        ] {
            sqlx::query(&format!("DELETE FROM {table} WHERE org_id = $1"))
                .bind(o)
                .execute(&mut admin)
                .await
                .expect("nettoyage");
        }
        sqlx::query("DELETE FROM organizations WHERE id = $1")
            .bind(o)
            .execute(&mut admin)
            .await
            .expect("nettoyage org");
    }

    let db = kollega_store::Db::connect(&app_url_from(&migrate_url))
        .await
        .expect("connexion applicative");
    for (o, nom) in [(org, "Org Tranche HTTP"), (voisine, "Org Voisine")] {
        let mut tx = db.org_transaction(o).await.expect("tx");
        sqlx::query("INSERT INTO organizations (id, name) VALUES ($1, $2)")
            .bind(o)
            .bind(nom)
            .execute(&mut *tx)
            .await
            .expect("organisation");
        sqlx::query("INSERT INTO credits (org_id, balance_cents) VALUES ($1, 10000)")
            .bind(o)
            .execute(&mut *tx)
            .await
            .expect("crédit");
        tx.commit().await.expect("commit");
    }

    // Le monde extérieur : il survit aux redémarrages du serveur.
    let outils: Arc<CountingTools> = Arc::new(CountingTools::default());

    // ---- Serveur n°1 --------------------------------------------------------
    let serveur = Serveur::demarrer(
        db.clone(),
        Some(Agent::new(
            Arc::new(relance_script()),
            Arc::clone(&outils) as Arc<dyn ToolRunner + Send + Sync>,
            relance_rules(),
        )),
    )
    .await;
    let addr = serveur.addr;

    // 1. CRÉÉE, par HTTP.
    let tache = Uuid::from_u128(0x00_5E_11_10);
    let reponse = poste(
        addr,
        &format!("/orgs/{org}/tasks/{tache}"),
        r#"{"ceiling_cents":10000,"max_iterations":4}"#,
    )
    .await;
    assert!(
        a_le_statut(&reponse, 201),
        "la création devait répondre 201 :\n{reponse}"
    );

    // 2. UN PAS : la politique voit l'appel COMPLET et exige une validation.
    //    Rien n'est facturé tant que le dirigeant n'a pas tranché.
    let reponse = poste(addr, &format!("/orgs/{org}/tasks/{tache}/steps"), "{}").await;
    assert!(
        a_le_statut(&reponse, 200),
        "le premier pas devait répondre 200 :\n{reponse}"
    );
    let vu: serde_json::Value = serde_json::from_str(corps(&reponse)).expect("réponse JSON");
    assert_eq!(
        vu["status"], "waiting_approval",
        "la politique suspend l'appel : {vu}"
    );
    assert_eq!(
        vu["balance_cents"], 10_000,
        "rien n'est facturé avant validation : {vu}"
    );
    assert_eq!(outils.count(), 0, "aucun mail avant validation");

    // 3. INTERRUPTION RÉELLE : le serveur s'arrête, son pool se ferme, tout
    //    ce qu'il avait en mémoire disparaît.
    serveur.arreter().await;
    drop(db);

    // 4. REPRISE DEPUIS LA BASE : nouveau pool, nouveau routeur, nouvel agent.
    //    Rien n'a été transmis d'un serveur à l'autre, hormis PostgreSQL.
    let db = kollega_store::Db::connect(&app_url_from(&migrate_url))
        .await
        .expect("reconnexion après interruption");
    let serveur = Serveur::demarrer(
        db.clone(),
        Some(Agent::new(
            Arc::new(relance_script()),
            Arc::clone(&outils) as Arc<dyn ToolRunner + Send + Sync>,
            relance_rules(),
        )),
    )
    .await;
    let addr = serveur.addr;

    // 5. VALIDÉE, EXÉCUTÉE, DÉBITÉE — par HTTP, sur le serveur qui vient de
    //    naître et qui n'a jamais vu le pas précédent.
    let reponse = poste(
        addr,
        &format!("/orgs/{org}/tasks/{tache}/steps"),
        r#"{"approval":"approve"}"#,
    )
    .await;
    assert!(
        a_le_statut(&reponse, 200),
        "la reprise devait répondre 200 :\n{reponse}"
    );
    let vu: serde_json::Value = serde_json::from_str(corps(&reponse)).expect("réponse JSON");
    assert_eq!(vu["status"], "succeeded", "la tâche aboutit : {vu}");
    assert_eq!(vu["conclusion"], "relance envoyée", "{vu}");
    assert_eq!(
        vu["balance_cents"],
        10_000 - 55,
        "30+20 approuvés, puis 5 de conclusion : {vu}"
    );
    assert_eq!(outils.count(), 1, "UN seul mail est parti");

    // 6. JOURNALISÉE EN BASE : la séquence complète des attestations, écrite
    //    par deux processus serveurs différents, forme une seule histoire.
    let mut tx = db.org_transaction(org).await.expect("tx lecture");
    let actions: Vec<String> =
        sqlx::query("SELECT action FROM audit_chain WHERE actor = $1 ORDER BY height")
            .bind(tache.to_string())
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
        "la chaîne raconte le parcours entier, à cheval sur le redémarrage"
    );
    drop(tx);

    kollega_store::driver::verify_org_chain(&db, org)
        .await
        .expect("la chaîne écrite par HTTP est intègre");
    let rapport = kollega_store::driver::verify_org_sequence(&db, org)
        .await
        .expect("séquence");
    assert!(
        rapport.is_valid(),
        "la séquence produite par le serveur est cohérente : {:?}",
        rapport.violations
    );
    assert!(
        rapport.open_calls.is_empty(),
        "la tâche est terminée : aucun appel ne reste ouvert : {:?}",
        rapport.open_calls
    );

    // 7. MÊME RÉSULTAT QU'UN PARCOURS DIRECT, sans redémarrage.
    let directe = Uuid::from_u128(0x00_5E_11_11);
    let reponse = poste(
        addr,
        &format!("/orgs/{org}/tasks/{directe}"),
        r#"{"ceiling_cents":10000,"max_iterations":4}"#,
    )
    .await;
    assert!(a_le_statut(&reponse, 201), "{reponse}");
    let reponse = poste(addr, &format!("/orgs/{org}/tasks/{directe}/steps"), "{}").await;
    assert!(a_le_statut(&reponse, 200), "{reponse}");
    let reponse = poste(
        addr,
        &format!("/orgs/{org}/tasks/{directe}/steps"),
        r#"{"approval":"approve"}"#,
    )
    .await;
    let vu: serde_json::Value = serde_json::from_str(corps(&reponse)).expect("réponse JSON");
    assert_eq!(
        vu["status"], "succeeded",
        "le parcours direct aboutit pareil : {vu}"
    );
    assert_eq!(vu["conclusion"], "relance envoyée", "{vu}");
    assert_eq!(
        vu["balance_cents"],
        10_000 - 110,
        "le même coût, à l'identique : {vu}"
    );
    assert_eq!(outils.count(), 2, "un second mail, pour une seconde tâche");

    // 8. Faire avancer la tâche d'une AUTRE organisation : `404`, comme une
    //    tâche inexistante. La RLS la rend invisible, et la réponse ne dit
    //    pas qu'elle existe ailleurs.
    let reponse = poste(addr, &format!("/orgs/{voisine}/tasks/{tache}/steps"), "{}").await;
    assert!(
        a_le_statut(&reponse, 404),
        "on ne fait pas avancer la tâche du voisin, et on ne l'apprend pas :\n{reponse}"
    );

    serveur.arreter().await;

    // 9. LE SERVEUR TEL QUE LE BINAIRE LE DÉMARRE : sans agent branché.
    //    `kollega serve` construit exactement ce routeur-là aujourd'hui. La
    //    route existe, et elle dit ce qu'elle ne sait pas faire — plutôt que
    //    de laisser croire qu'un modèle est branché.
    let nu = Serveur::demarrer(db.clone(), None).await;
    let reponse = poste(nu.addr, &format!("/orgs/{org}/tasks/{tache}/steps"), "{}").await;
    assert!(
        a_le_statut(&reponse, 503),
        "sans fournisseur de modèle, le pas est indisponible :\n{reponse}"
    );
    nu.arreter().await;
}
