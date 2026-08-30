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
use crate::scanjob::ScanRegistry;
use crate::security::OriginPolicy;
use crate::{
    auth, bootstrap, health, items, library, observability, security, thumbnails, transfers,
};

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
    origin_policy: OriginPolicy,
    login_attempts: AttemptLimiter,
    scans: Arc<ScanRegistry>,
}

impl AppState {
    pub fn new(db: PgPool, storage_root: PathBuf, production: bool) -> Self {
        Self::with_origins(db, storage_root, production, Vec::new())
    }

    pub fn with_origins(
        db: PgPool,
        storage_root: PathBuf,
        production: bool,
        trusted_origins: Vec<String>,
    ) -> Self {
        Self {
            db,
            inner: Arc::new(Inner {
                storage_root,
                secure_cookies: production,
                origin_policy: OriginPolicy {
                    trusted: trusted_origins,
                    // Development proxies the web app from another port;
                    // production must name its origin explicitly.
                    allow_loopback: !production,
                },
                login_attempts: AttemptLimiter::new(),
                scans: Arc::new(ScanRegistry::new()),
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

    pub fn scans(&self) -> &Arc<ScanRegistry> {
        &self.inner.scans
    }

    pub fn origin_policy(&self) -> &OriginPolicy {
        &self.inner.origin_policy
    }
}

/// Builds the application router.
pub fn router(state: AppState) -> Router {
    let security_state = state.clone();

    let metadata = Router::new()
        .route("/health/live", get(health::live))
        .route("/health/ready", get(health::ready))
        .route("/api/v1/bootstrap", get(bootstrap::status))
        .route("/api/v1/setup", post(auth::setup))
        .route("/api/v1/auth/login", post(auth::login))
        .route("/api/v1/auth/logout", post(auth::logout))
        .route("/api/v1/session", get(auth::session_status))
        .route("/api/v1/libraries", get(library::list))
        .route("/api/v1/libraries/{library}/browse", get(library::browse))
        .route("/api/v1/libraries/{library}/photos", get(library::photos))
        .route("/api/v1/libraries/{library}/search", get(library::search))
        .route("/api/v1/libraries/{library}/trash", get(library::trash))
        .route(
            "/api/v1/libraries/{library}/scan",
            get(library::scan_status).post(library::start_scan),
        )
        .route(
            "/api/v1/libraries/{library}/folders",
            post(items::create_folder),
        )
        .route(
            "/api/v1/items/{item}",
            get(items::get).delete(items::trash_item),
        )
        .route("/api/v1/items/{item}/children", get(items::children))
        .route("/api/v1/items/{item}/move", post(items::move_item))
        .route("/api/v1/items/{item}/restore", post(items::restore_item))
        .route("/api/v1/items/{item}/content", get(transfers::download))
        .route("/api/v1/items/{item}/thumbnail", get(thumbnails::thumbnail))
        .fallback(not_found)
        // Metadata bodies are small; anything larger is a mistake or an
        // attack, and is rejected before a handler sees it.
        .layer(RequestBodyLimitLayer::new(
            security::MAX_METADATA_BODY_BYTES,
        ));

    // Transfers get their own, much larger, bound: a metadata limit of
    // 64 KiB would make the product useless, and one shared limit of
    // gigabytes would make every metadata route a memory risk.
    let transfers = Router::new()
        .route(
            "/api/v1/libraries/{library}/upload",
            post(transfers::upload),
        )
        .layer(RequestBodyLimitLayer::new(
            transfers::MAX_UPLOAD_BYTES as usize,
        ));

    metadata
        .merge(transfers)
        .layer(axum::middleware::from_fn_with_state(
            security_state,
            security::security_middleware,
        ))
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
