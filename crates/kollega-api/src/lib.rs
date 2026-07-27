//! Couche HTTP (axum) : `GET /health`, création et lecture d'une tâche, et
//! le PAS qui la fait avancer.
//!
//! L'accès aux données passe exclusivement par `kollega_store::Db` — le point
//! de passage unique qui pose le contexte d'organisation (invariant 1).

#![forbid(unsafe_code)]

pub mod auth;

use std::sync::Arc;

use axum::extract::{FromRef, Path, State};
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router};
use kollega_policy::ToolRule;
use kollega_runtime::machine::{ApprovalDecision, ModelProvider, ToolRunner};
use kollega_store::{Db, StoreError};

/// Ce qu'il faut au serveur pour faire AVANCER une tâche : un fournisseur de
/// plan, un exécuteur d'outils, et les règles de l'organisation.
///
/// Les trois sont injectés, jamais construits ici. C'est la même méthode que
/// l'attente d'arrêt de [`serve_until`] : le chemin exercé par un test est
/// exactement celui qu'emprunte le binaire, seule la pièce branchée au bout
/// change.
///
/// # Pourquoi c'est une OPTION dans l'état du serveur
///
/// Quel `ModelProvider` réel brancher — et comment la boucle recevra
/// l'estimation de jetons que `ModelRequest` porte déjà — engage la
/// conception de la boucle d'agent, et appartient au propriétaire
/// (`docs/questions-nuit.md`). Tant que cette décision n'est pas prise, le
/// binaire démarre SANS agent et la route de pas répond `503`. Câbler ici un
/// modèle de démonstration donnerait un serveur qui a l'air de fonctionner en
/// produisant des plans qui ne viennent d'aucun modèle : c'est exactement le
/// genre de vert trompeur que ce dépôt refuse.
#[derive(Clone)]
pub struct Agent {
    model: Arc<dyn ModelProvider + Send + Sync>,
    tools: Arc<dyn ToolRunner + Send + Sync>,
    rules: Arc<[ToolRule]>,
}

impl Agent {
    /// Assemble un agent à partir de ses trois pièces.
    #[must_use]
    pub fn new(
        model: Arc<dyn ModelProvider + Send + Sync>,
        tools: Arc<dyn ToolRunner + Send + Sync>,
        rules: Vec<ToolRule>,
    ) -> Self {
        Agent {
            model,
            tools,
            rules: rules.into(),
        }
    }
}

/// État partagé du serveur.
#[derive(Clone)]
struct AppState {
    db: Db,
    agent: Option<Agent>,
}

// Les handlers qui ne touchent qu'à la base continuent d'extraire `Db` seul :
// ils n'ont aucune raison de voir l'agent.
impl FromRef<AppState> for Db {
    fn from_ref(state: &AppState) -> Db {
        state.db.clone()
    }
}

/// Construit le routeur HTTP de l'application.
///
/// `agent` vaut `None` tant qu'aucun fournisseur de modèle n'est branché : la
/// route de pas existe alors quand même et répond `503`. Le routeur est le
/// même dans les deux cas — un test exerce donc le routeur du produit, pas un
/// montage parallèle.
pub fn router(db: Db, agent: Option<Agent>) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/orgs/{org}/tasks/{task}", post(create_task).get(read_task))
        .route("/orgs/{org}/tasks/{task}/steps", post(advance_task))
        .with_state(AppState { db, agent })
}

/// Corps d'une demande de pas : la décision humaine, s'il y en a une.
#[derive(serde::Deserialize)]
pub struct StepRequest {
    /// Décision sur une action suspendue. Absente = simple avancée.
    #[serde(default)]
    pub approval: Option<Approval>,
}

/// Décision humaine, dans la forme exposée par HTTP.
///
/// Distincte de `ApprovalDecision` du runtime, et convertie explicitement :
/// la forme du contrat HTTP ne doit pas suivre les renommages internes d'un
/// type de domaine.
#[derive(serde::Deserialize, Clone, Copy)]
#[serde(rename_all = "snake_case")]
pub enum Approval {
    /// L'action suspendue est approuvée.
    Approve,
    /// L'action suspendue est refusée.
    Reject,
}

impl From<Approval> for ApprovalDecision {
    fn from(approval: Approval) -> ApprovalDecision {
        match approval {
            Approval::Approve => ApprovalDecision::Approve,
            Approval::Reject => ApprovalDecision::Reject,
        }
    }
}

/// UN PAS de tâche, par HTTP — la tranche verticale traversée par le binaire.
///
/// Relecture de l'état en base, politique, exécution éventuelle, débit,
/// attestations, écriture : tout se passe dans `driver::run_task_step`, dans
/// une transaction posée sur `Db::org_transaction`. Ce handler ne fait que
/// transporter — il ne décide de rien et n'écrit aucune ligne lui-même.
async fn advance_task(
    State(state): State<AppState>,
    Path((org, task)): Path<(uuid::Uuid, uuid::Uuid)>,
    Json(body): Json<StepRequest>,
) -> (StatusCode, String) {
    let Some(agent) = state.agent else {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            "aucun fournisseur de modèle n'est branché sur ce serveur".to_owned(),
        );
    };
    match kollega_store::driver::run_task_step(
        &state.db,
        org,
        task,
        agent.model.as_ref(),
        agent.tools.as_ref(),
        &agent.rules,
        body.approval.map(ApprovalDecision::from),
    )
    .await
    {
        Ok(step) => {
            let corps = serde_json::json!({
                "status": step.status,
                "balance_cents": step.org_balance.0,
                "conclusion": step.conclusion,
            });
            (StatusCode::OK, corps.to_string())
        }
        // Une tâche absente du contexte d'organisation courant est
        // indiscernable d'une tâche inexistante — même raisonnement que pour
        // la lecture : distinguer les deux permettrait de les énumérer.
        Err(StoreError::TaskNotFound) => (StatusCode::NOT_FOUND, "inconnue".to_owned()),
        Err(e) => {
            tracing::error!(erreur = %e, "pas de tâche impossible");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "pas impossible".to_owned(),
            )
        }
    }
}

/// Corps de création d'une tâche.
#[derive(serde::Deserialize)]
pub struct NewTask {
    /// Plafond de coût de la tâche, en centimes.
    pub ceiling_cents: i64,
    /// Nombre maximal d'itérations.
    pub max_iterations: u32,
}

/// ÉCRITURE réelle à travers le point de passage unique.
///
/// # L'organisation vient du CHEMIN, et c'est provisoire
///
/// Il n'y a pas encore de session : l'authentification est le jalon M1. En
/// attendant, l'organisation est prise dans l'URL — ce qui signifie qu'un
/// client peut désigner n'importe laquelle. **Cela ne doit jamais atteindre
/// la production en l'état** : l'organisation devra venir de la session,
/// jamais de la requête.
///
/// Ce qui limite déjà les dégâts, et qu'il faut connaître pour ne pas s'en
/// contenter : la RLS ne rend visibles que les lignes de l'organisation
/// posée dans le contexte. Un client qui désigne l'organisation d'un autre
/// n'obtient donc pas ses données — il peut en revanche ÉCRIRE chez elle,
/// puisque le contexte qu'il a choisi devient le sien. C'est exactement la
/// raison pour laquelle ce raccourci est un échafaudage, pas une solution.
async fn create_task(
    State(db): State<Db>,
    Path((org, task)): Path<(uuid::Uuid, uuid::Uuid)>,
    axum::Json(body): axum::Json<NewTask>,
) -> (StatusCode, &'static str) {
    match kollega_store::driver::create_task(
        &db,
        org,
        task,
        kollega_core::Cents(body.ceiling_cents),
        body.max_iterations,
    )
    .await
    {
        Ok(()) => (StatusCode::CREATED, "créée"),
        Err(e) => {
            tracing::error!(erreur = %e, "création de tâche impossible");
            (StatusCode::UNPROCESSABLE_ENTITY, "création impossible")
        }
    }
}

/// LECTURE réelle à travers le point de passage unique.
///
/// Une tâche d'une autre organisation rend `404`, comme une tâche
/// inexistante : les deux sont indiscernables par construction, ce qui
/// interdit d'énumérer les tâches d'autrui en observant les réponses.
async fn read_task(
    State(db): State<Db>,
    Path((org, task)): Path<(uuid::Uuid, uuid::Uuid)>,
) -> (StatusCode, String) {
    match kollega_store::driver::read_task_status(&db, org, task).await {
        Ok(Some(status)) => (StatusCode::OK, format!("{status:?}")),
        Ok(None) => (StatusCode::NOT_FOUND, "inconnue".to_owned()),
        Err(e) => {
            tracing::error!(erreur = %e, "lecture de tâche impossible");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "lecture impossible".to_owned(),
            )
        }
    }
}

/// Sert jusqu'à ce que `shutdown` s'achève, puis s'arrête PROPREMENT.
///
/// Sans cela, `axum::serve(...).await` ne rend la main que sur erreur : un
/// `SIGTERM` — ce qu'envoient `docker stop`, systemd et l'ordonnanceur d'un
/// hébergeur au moment d'un redéploiement — tuait le processus au milieu
/// des requêtes en vol. Pour un produit dont la valeur est un journal
/// d'audit, couper une requête entre l'attestation d'intention et celle de
/// résultat laisse dans la chaîne un appel ouvert qu'aucune reprise ne
/// réclamera.
///
/// L'attente d'arrêt est un paramètre, pas un appel direct au système : le
/// binaire y branche les signaux, un test y branche un canal, et le chemin
/// exercé est le même.
///
/// # Errors
///
/// Rend l'erreur d'`axum::serve` si le service s'interrompt autrement que
/// par l'arrêt demandé.
pub async fn serve_until(
    listener: tokio::net::TcpListener,
    app: Router,
    shutdown: impl std::future::Future<Output = ()> + Send + 'static,
) -> std::io::Result<()> {
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown)
        .await
}

/// Vérifie réellement la connexion à la base : un `SELECT 1` doit aboutir.
async fn health(State(db): State<Db>) -> (StatusCode, &'static str) {
    match db.health().await {
        Ok(()) => (StatusCode::OK, "ok"),
        Err(_) => (
            StatusCode::SERVICE_UNAVAILABLE,
            "base de données injoignable",
        ),
    }
}

// Le `fn crate_compiles() {}` qui vivait ici a été retiré le 29/07 : il ne
// prouvait rien de plus que le compilateur, tout en comptant comme un test
// vert. La couche HTTP est désormais éprouvée par `tests/health_endpoint.rs`,
// qui démarre un vrai serveur sur une vraie socket.
