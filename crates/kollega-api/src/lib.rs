//! Couche HTTP (axum). Au jalon M0 : `GET /health`, rien d'autre.

#![forbid(unsafe_code)]

use axum::extract::State;
use axum::http::StatusCode;
use axum::routing::get;
use axum::Router;
use sqlx::PgPool;

/// Construit le routeur HTTP de l'application.
pub fn router(pool: PgPool) -> Router {
    Router::new().route("/health", get(health)).with_state(pool)
}

/// Vérifie réellement la connexion à la base : un `SELECT 1` doit aboutir.
async fn health(State(pool): State<PgPool>) -> (StatusCode, &'static str) {
    match sqlx::query("SELECT 1").execute(&pool).await {
        Ok(_) => (StatusCode::OK, "ok"),
        Err(_) => (
            StatusCode::SERVICE_UNAVAILABLE,
            "base de données injoignable",
        ),
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn crate_compiles() {}
}
