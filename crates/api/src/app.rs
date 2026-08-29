//! Router construction.
//!
//! The router is built from injected state so tests exercise exactly the
//! application the binary serves, without starting a process.

use axum::routing::get;
use axum::Router;
use sqlx::PgPool;

use crate::error::ApiError;
use crate::health;

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
        .with_state(state)
}

/// Unknown routes return the same problem shape as every other error, so
/// clients never have to parse two error formats.
async fn not_found() -> ApiError {
    ApiError::not_found()
}
