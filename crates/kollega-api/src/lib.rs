//! Couche HTTP (axum). Au jalon M0 : `GET /health`, rien d'autre.
//!
//! L'accès aux données passe exclusivement par `kollega_store::Db` — le point
//! de passage unique qui pose le contexte d'organisation (invariant 1).

#![forbid(unsafe_code)]

pub mod auth;

use axum::extract::State;
use axum::http::StatusCode;
use axum::routing::get;
use axum::Router;
use kollega_store::Db;

/// Construit le routeur HTTP de l'application.
pub fn router(db: Db) -> Router {
    Router::new().route("/health", get(health)).with_state(db)
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
