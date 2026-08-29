//! Router and application state.
//!
//! The router is built from injected state so tests exercise exactly the
//! application the binary serves, without starting a process.

use std::path::PathBuf;
use std::sync::Arc;

use axum::routing::{get, post};
use axum::Router;
use sqlx::PgPool;
use tower_http::catch_panic::CatchPanicLayer;
use tower_http::limit::RequestBodyLimitLayer;

use crate::error::ApiError;
use crate::ratelimit::AttemptLimiter;
use crate::{auth, bootstrap, health, observability, security};

/// Everything a handler is allowed to reach. Cheap to clone: the pool is
/// internally reference-counted and the rest is shared behind an `Arc`.
#[derive(Debug, Clone)]
pub struct AppState {
    db: PgPool,
    inner: Arc<Inner>,
}

#[derive(Debug)]
struct Inner {
    storage_root: PathBuf,
    secure_cookies: bool,
    login_attempts: AttemptLimiter,
}

impl AppState {
    pub fn new(db: PgPool, storage_root: PathBuf, secure_cookies: bool) -> Self {
        Self {
            db,
            inner: Arc::new(Inner {
                storage_root,
                secure_cookies,
                login_attempts: AttemptLimiter::new(),
            }),
        }
    }

    pub fn db(&self) -> &PgPool {
        &self.db
    }

    pub fn storage_root(&self) -> &std::path::Path {
        &self.inner.storage_root
    }

    /// The configured root as text, for storing on a library row.
    pub fn storage_root_display(&self) -> String {
        self.inner.storage_root.to_string_lossy().into_owned()
    }

    /// Whether session cookies may be marked `Secure`. A `Secure` cookie
    /// is not sent over plain HTTP, which would break a loopback-only
    /// first run.
    pub fn secure_cookies(&self) -> bool {
        self.inner.secure_cookies
    }

    pub fn login_attempts(&self) -> &AttemptLimiter {
        &self.inner.login_attempts
    }
}

/// Builds the application router.
pub fn router(state: AppState) -> Router {
    let metadata = Router::new()
        .route("/health/live", get(health::live))
        .route("/health/ready", get(health::ready))
        .route("/api/v1/bootstrap", get(bootstrap::status))
        .route("/api/v1/setup", post(auth::setup))
        .route("/api/v1/auth/login", post(auth::login))
        .route("/api/v1/auth/logout", post(auth::logout))
        .route("/api/v1/session", get(auth::session_status))
        .fallback(not_found)
        // Metadata bodies are small; anything larger is a mistake or an
        // attack, and is rejected before a handler sees it.
        .layer(RequestBodyLimitLayer::new(
            security::MAX_METADATA_BODY_BYTES,
        ));

    metadata
        .layer(axum::middleware::from_fn(security::security_middleware))
        // Panics become a problem response first, then the correlation
        // layer wraps everything so even a panic carries a request id.
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
