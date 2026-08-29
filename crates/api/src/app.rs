//! Router construction.
//!
//! The router is built from injected state so tests exercise exactly the
//! application the binary serves, without starting a process.

use axum::routing::get;
use axum::Router;
use sqlx::PgPool;
use tower_http::catch_panic::CatchPanicLayer;

use crate::error::ApiError;
use crate::health;
use crate::observability;

/// Everything a handler is allowed to reach. Cheap to clone: the pool is
/// internally reference-counted.
#[derive(Debug, Clone)]
pub struct AppState {
    db: PgPool,
}

impl AppState {
    pub fn new(db: PgPool) -> Self {
        Self { db }
    }

    pub fn db(&self) -> &PgPool {
        &self.db
    }
}

/// Builds the application router.
pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/health/live", get(health::live))
        .route("/health/ready", get(health::ready))
        .fallback(not_found)
        // Panics are converted to a problem response first, then the
        // correlation layer wraps everything so even a panic response
        // carries a request id.
        .layer(CatchPanicLayer::custom(observability::panic_response))
        .layer(axum::middleware::from_fn(
            observability::request_id_middleware,
        ))
        .with_state(state)
}

/// Unknown routes return the same problem shape as every other error, so
/// clients never have to parse two error formats.
async fn not_found() -> ApiError {
    ApiError::not_found()
}
