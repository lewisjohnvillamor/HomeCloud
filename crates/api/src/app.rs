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
use crate::passkeys::Ceremonies;
use crate::ratelimit::AttemptLimiter;
use crate::scanjob::ScanRegistry;
use crate::security::OriginPolicy;
use crate::{
    auth, bootstrap, health, items, library, members, observability, passkeys, security, shares,
    thumbnails, transfers,
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
    passkeys: Option<homecloud_auth::PasskeyService>,
    ceremonies: Ceremonies,
}

/// How the server is deployed. Grouped rather than passed as a handful
/// of positional arguments, so adding a setting does not ripple through
/// every construction site.
#[derive(Debug, Clone)]
pub struct AppSettings {
    pub storage_root: PathBuf,
    /// Production tightens cookies and the cross-origin policy.
    pub production: bool,
    /// Origins allowed to make state-changing requests, for a proxy that
    /// rewrites `Host`.
    pub trusted_origins: Vec<String>,
    /// The address people reach this server at, such as
    /// `https://home.example`. Passkeys are bound to it, so without it
    /// they are unavailable rather than guessed.
    pub public_origin: Option<String>,
}

impl AppSettings {
    /// Settings for a local development or test server.
    pub fn development(storage_root: PathBuf) -> Self {
        Self {
            storage_root,
            production: false,
            trusted_origins: Vec::new(),
            public_origin: None,
        }
    }
}

impl AppState {
    pub fn new(db: PgPool, settings: AppSettings) -> Self {
        // A public origin that cannot be parsed disables passkeys rather
        // than failing startup: the rest of the server still works, and
        // the reason is logged once here.
        let passkeys = settings.public_origin.as_deref().and_then(|origin| {
            match homecloud_auth::PasskeyService::new(origin) {
                Ok(service) => Some(service),
                Err(error) => {
                    tracing::error!(error = %error, "passkeys are disabled: the public origin is not usable");
                    None
                }
            }
        });

        let mut trusted = settings.trusted_origins;
        if let Some(origin) = settings.public_origin.clone() {
            trusted.push(origin);
        }

        Self {
            db,
            inner: Arc::new(Inner {
                storage_root: settings.storage_root,
                secure_cookies: settings.production,
                origin_policy: OriginPolicy {
                    trusted,
                    // Development proxies the web app from another port;
                    // production must name its origin explicitly.
                    allow_loopback: !settings.production,
                },
                login_attempts: AttemptLimiter::new(),
                scans: Arc::new(ScanRegistry::new()),
                passkeys,
                ceremonies: Ceremonies::new(),
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

    /// The passkey service, or `None` when this server has no public
    /// origin configured and therefore cannot do WebAuthn.
    pub fn passkeys(&self) -> Option<&homecloud_auth::PasskeyService> {
        self.inner.passkeys.as_ref()
    }

    pub fn ceremonies(&self) -> &Ceremonies {
        &self.inner.ceremonies
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
        .route(
            "/api/v1/auth/passkeys",
            get(passkeys::list).post(passkeys::register_options),
        )
        .route(
            "/api/v1/auth/passkeys/register/options",
            post(passkeys::register_options),
        )
        .route(
            "/api/v1/auth/passkeys/register/verify",
            post(passkeys::register_verify),
        )
        .route(
            "/api/v1/auth/passkeys/login/options",
            post(passkeys::login_options),
        )
        .route(
            "/api/v1/auth/passkeys/login/verify",
            post(passkeys::login_verify),
        )
        .route(
            "/api/v1/auth/passkeys/{credential}",
            axum::routing::delete(passkeys::remove),
        )
        .route("/api/v1/libraries", get(library::list))
        .route("/api/v1/libraries/{library}/browse", get(library::browse))
        .route("/api/v1/libraries/{library}/photos", get(library::photos))
        .route(
            "/api/v1/libraries/{library}/memories",
            get(library::memories),
        )
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
        .route(
            "/api/v1/items/{item}/shares",
            get(shares::list_for_item).post(shares::create),
        )
        .route(
            "/api/v1/shares/{share}",
            axum::routing::delete(shares::revoke),
        )
        .route(
            "/api/v1/libraries/{library}/shares",
            get(shares::list_for_library),
        )
        .route(
            "/api/v1/libraries/{library}/members",
            get(members::list_members),
        )
        .route(
            "/api/v1/libraries/{library}/members/{member}",
            axum::routing::delete(members::remove_member),
        )
        .route(
            "/api/v1/libraries/{library}/invitations",
            get(members::list_invitations).post(members::create_invitation),
        )
        .route(
            "/api/v1/invitations/{invitation}",
            axum::routing::delete(members::revoke_invitation),
        )
        // Accepting an invitation cannot require a session: the person
        // accepting usually does not have an account yet.
        .route(
            "/api/v1/invitations/{token}/preview",
            get(members::preview_invitation),
        )
        .route(
            "/api/v1/invitations/{token}/accept",
            post(members::accept_invitation),
        )
        // Public share routes take no session: the token in the path is
        // the entire credential, and it grants read access to one item.
        .route("/api/v1/public/{token}", get(shares::public_view))
        .route(
            "/api/v1/public/{token}/content",
            get(shares::public_content),
        )
        .route(
            "/api/v1/public/{token}/thumbnail",
            get(shares::public_thumbnail),
        )
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
