//! Couche HTTP (axum). Au jalon M0 : `GET /health`, rien d'autre.
//!
//! L'accès aux données passe exclusivement par `kollega_store::Db` — le point
//! de passage unique qui pose le contexte d'organisation (invariant 1).

#![forbid(unsafe_code)]

pub mod auth;

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::Router;
use kollega_store::Db;

/// Construit le routeur HTTP de l'application.
pub fn router(db: Db) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/orgs/{org}/tasks/{task}", post(create_task).get(read_task))
        .with_state(db)
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
