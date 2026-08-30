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
    ai, albums, auth, bootstrap, health, items, library, members, observability, passkeys,
    recovery, requests, security, shares, thumbnails, transfers, tv, uploads, versions,
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
    share_unlocks: shares::ShareUnlocks,
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
                share_unlocks: shares::ShareUnlocks::new(),
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

    pub fn share_unlocks(&self) -> &shares::ShareUnlocks {
        &self.inner.share_unlocks
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
            "/api/v1/auth/recovery",
            get(recovery::status).post(recovery::regenerate),
        )
        // Recovering takes no session, by definition.
        .route("/api/v1/auth/recover", post(recovery::recover))
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
        .route("/api/v1/items/{item}/copy", post(items::copy_item))
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
        .route("/api/v1/public/{token}/unlock", post(shares::unlock))
        .route(
            "/api/v1/public/{token}/content",
            get(shares::public_content),
        )
        .route(
            "/api/v1/public/{token}/thumbnail",
            get(shares::public_thumbnail),
        )
        // The private AI switch. Off by default, owner-only to change.
        .route(
            "/api/v1/libraries/{library}/ai",
            get(ai::read).put(ai::update),
        )
        // Where photos were taken.
        .route("/api/v1/libraries/{library}/places", get(library::places))
        // Exact duplicates, for reclaiming space.
        .route(
            "/api/v1/libraries/{library}/duplicates",
            get(library::duplicates),
        )
        // What a file used to be. Replacing goes through the transfer
        // router below, which has the larger body limit.
        .route("/api/v1/items/{item}/versions", get(versions::list))
        .route(
            "/api/v1/items/{item}/versions/{version}/content",
            get(versions::download),
        )
        .route(
            "/api/v1/items/{item}/versions/{version}/restore",
            post(versions::restore),
        )
        // Upload request links: the mirror image of a share. Someone
        // with the link can write into one folder and read nothing.
        .route(
            "/api/v1/items/{item}/upload-requests",
            post(requests::create),
        )
        .route(
            "/api/v1/libraries/{library}/upload-requests",
            get(requests::list),
        )
        .route(
            "/api/v1/upload-requests/{id}",
            axum::routing::delete(requests::revoke),
        )
        .route(
            "/api/v1/public/upload-requests/{token}",
            get(requests::public_view),
        )
        // Resumable uploads. The bytes themselves go through the
        // transfer router below, which has the larger body limit.
        .route("/api/v1/uploads", post(uploads::create))
        .route(
            "/api/v1/uploads/{id}",
            get(uploads::status).delete(uploads::abort),
        )
        .route("/api/v1/uploads/{id}/complete", post(uploads::complete))
        .route("/api/v1/libraries/{library}/uploads", get(uploads::list))
        // Curating a library: one person's favorites, and albums the
        // whole library shares.
        .route(
            "/api/v1/items/{item}/favorite",
            axum::routing::put(albums::add_favorite).delete(albums::remove_favorite),
        )
        .route(
            "/api/v1/libraries/{library}/favorites",
            get(albums::list_favorites),
        )
        .route(
            "/api/v1/libraries/{library}/albums",
            get(albums::list_albums).post(albums::create_album),
        )
        .route(
            "/api/v1/albums/{album}",
            get(albums::read_album)
                .patch(albums::rename_album)
                .delete(albums::delete_album),
        )
        .route("/api/v1/albums/{album}/items", post(albums::add_to_album))
        .route(
            "/api/v1/albums/{album}/items/{item}",
            axum::routing::delete(albums::remove_from_album),
        )
        // Pairing a television. Starting and polling take no session:
        // a screen that cannot sign in is the whole reason this exists.
        // Approving one does, and is the deliberate human step.
        .route("/api/v1/tv/pairings", post(tv::start))
        .route("/api/v1/tv/pairings/{poll_token}", get(tv::poll))
        .route("/api/v1/tv/pairings/{code}/approve", post(tv::approve))
        .route("/api/v1/libraries/{library}/tv", get(tv::list))
        .route(
            "/api/v1/tv/devices/{device}",
            axum::routing::delete(tv::revoke),
        )
        // What a paired screen may read: one library's memories, and
        // only items that belong in a photo timeline.
        .route("/api/v1/tv/memories", get(tv::memories))
        .route("/api/v1/tv/thumbnail", get(tv::thumbnail))
        .route("/api/v1/tv/content", get(tv::content))
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
        // One chunk of a resumable upload. Bounded much lower than a
        // whole-file upload, because the point of a session is that no
        // single request has to carry the file.
        .route(
            "/api/v1/uploads/{id}",
            axum::routing::patch(uploads::append),
        )
        .route(
            "/api/v1/items/{item}/content",
            axum::routing::put(versions::replace),
        )
        // A file arriving through an upload request link. No session,
        // and the link's own limits bound what it can cost.
        .route(
            "/api/v1/public/upload-requests/{token}/files",
            post(requests::send),
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
