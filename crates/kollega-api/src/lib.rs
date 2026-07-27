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
