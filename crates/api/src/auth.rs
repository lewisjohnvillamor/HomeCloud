//! Authentication endpoints and the current-user extractor.
//!
//! Sessions live in an `HttpOnly` cookie, so the token is never readable
//! by page scripts. Authorization decisions happen here and in the
//! catalog service — never in the browser.

use axum::extract::{FromRequestParts, State};
use axum::http::request::Parts;
use axum::http::{header, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use homecloud_auth::session::{self, SESSION_TTL};
use homecloud_domain::identity::UserId;
use homecloud_domain::naming::LibraryName;
use serde::{Deserialize, Serialize};

use crate::app::AppState;
use crate::bootstrap;
use crate::error::ApiError;

/// Cookie name. The `__Host-` prefix is deliberately not used: it
/// requires HTTPS, and a first-run deployment on a home network is often
/// reached over plain HTTP on loopback before a proxy exists.
pub const SESSION_COOKIE: &str = "homecloud_session";

/// Builds the `Set-Cookie` value for a new session.
///
/// `SameSite=Lax` keeps the cookie off cross-site subrequests while
/// still surviving a normal top-level navigation into the app.
fn session_cookie(token: &str, secure: bool) -> Option<HeaderValue> {
    let max_age = SESSION_TTL.whole_seconds();
    let secure = if secure { "; Secure" } else { "" };

    HeaderValue::from_str(&format!(
        "{SESSION_COOKIE}={token}; Path=/; HttpOnly; SameSite=Lax; Max-Age={max_age}{secure}"
    ))
    .ok()
}

fn clearing_cookie(secure: bool) -> HeaderValue {
    let secure = if secure { "; Secure" } else { "" };

    HeaderValue::from_str(&format!(
        "{SESSION_COOKIE}=; Path=/; HttpOnly; SameSite=Lax; Max-Age=0{secure}"
    ))
    .expect("static cookie value is valid")
}

/// Reads one cookie out of a `Cookie` header without pulling in a cookie
/// jar: the server only ever sets one cookie.
fn cookie_value<'a>(header: &'a str, name: &str) -> Option<&'a str> {
    header.split(';').find_map(|pair| {
        let (key, value) = pair.split_once('=')?;
        (key.trim() == name).then(|| value.trim())
    })
}

/// The authenticated user. Extracting it is what makes a route require
/// authentication, so a route that forgets to ask cannot leak data.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CurrentUser(pub UserId);

impl FromRequestParts<AppState> for CurrentUser {
    type Rejection = ApiError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let token = parts
            .headers
            .get(header::COOKIE)
            .and_then(|value| value.to_str().ok())
            .and_then(|header| cookie_value(header, SESSION_COOKIE))
            .ok_or_else(ApiError::unauthenticated)?;

        match session::authenticate(state.db(), token).await {
            Ok(Some(session)) => Ok(Self(session.user)),
            Ok(None) => Err(ApiError::unauthenticated()),
            Err(error) => {
                tracing::warn!(error = %error, "session lookup failed");
                Err(ApiError::dependency_unavailable("database"))
            }
        }
    }
}

/// Resolves the caller from a request's cookies, without making the
/// route require a session.
///
/// For handlers that behave differently for a signed-in visitor but are
/// still open to anyone — accepting an invitation, for instance.
pub(crate) async fn current_user(state: &AppState, parts: &Parts) -> Option<UserId> {
    let token = parts
        .headers
        .get(header::COOKIE)
        .and_then(|value| value.to_str().ok())
        .and_then(|header| cookie_value(header, SESSION_COOKIE))?;

    match session::authenticate(state.db(), token).await {
        Ok(session) => session.map(|session| session.user),
        Err(error) => {
            tracing::warn!(error = %error, "session lookup failed");
            None
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct SetupRequest {
    pub display_name: String,
    pub password: String,
    pub library_name: String,
}

#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    pub display_name: String,
    pub password: String,
}

#[derive(Debug, Serialize)]
pub struct SessionResponse {
    pub authenticated: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
}

/// `POST /api/v1/setup` — creates the owner account on a fresh
/// deployment and signs it in. Refused once an owner exists, so this
/// cannot be used to take over a running deployment.
pub async fn setup(
    State(state): State<AppState>,
    Json(request): Json<SetupRequest>,
) -> Result<Response, ApiError> {
    let display_name = request.display_name.trim().to_owned();
    if display_name.is_empty() || display_name.chars().count() > 64 {
        return Err(ApiError::bad_request("Enter a name of 1 to 64 characters."));
    }

    let library_name = LibraryName::parse(&request.library_name)
        .map_err(|error| ApiError::bad_request(error.to_string()))?;

    homecloud_auth::password::check_policy(&request.password)
        .map_err(|error| ApiError::bad_request(error.to_string()))?;
    let password_hash = homecloud_auth::hash_password(request.password)
        .await
        .map_err(|_| ApiError::internal())?;

    let owner = bootstrap::create_owner(
        state.db(),
        &display_name,
        &password_hash,
        &library_name,
        state.storage_root_display(),
    )
    .await
    .map_err(|error| match error {
        bootstrap::BootstrapError::AlreadyInitialised => {
            ApiError::conflict("This deployment already has an owner.")
        }
        bootstrap::BootstrapError::Database(error) => {
            if is_unique_violation(&error) {
                return ApiError::conflict("An account with that name already exists.");
            }

            tracing::error!(error = %error, "owner creation failed");
            ApiError::internal()
        }
    })?;

    // A deployment pointed at an existing folder should show its files
    // immediately, so the first scan starts as soon as the owner exists.
    match crate::library::storage_for(&state, owner.library).await {
        Ok(storage) => {
            state
                .scans()
                .start(owner.library, state.db().clone(), storage);
        }
        Err(error) => {
            // Setup itself still succeeded; the owner can scan manually.
            tracing::warn!(error = ?error.code(), "initial scan could not be started");
        }
    }

    issue_session(&state, owner.user, display_name).await
}

/// `POST /api/v1/auth/login`
pub async fn login(
    State(state): State<AppState>,
    Json(request): Json<LoginRequest>,
) -> Result<Response, ApiError> {
    let display_name = request.display_name.trim().to_owned();

    // One shared failure for "no such user" and "wrong password": the
    // login form must not report which accounts exist.
    let invalid = || ApiError::unauthorized("That name and password do not match.");

    if let Err(retry_after) = state.login_attempts().check(&display_name) {
        return Err(ApiError::too_many_requests(retry_after));
    }

    let row: Option<(uuid::Uuid, Option<String>)> =
        sqlx::query_as("SELECT id, password_hash FROM users WHERE lower(display_name) = lower($1)")
            .bind(&display_name)
            .fetch_optional(state.db())
            .await
            .map_err(|error| {
                tracing::warn!(error = %error, "login lookup failed");
                ApiError::dependency_unavailable("database")
            })?;

    let Some((user_id, Some(password_hash))) = row else {
        // Still spend time hashing so a missing account is not
        // distinguishable by response time.
        homecloud_auth::verify_password(request.password, dummy_hash()).await;
        state.login_attempts().record_failure(&display_name);
        return Err(invalid());
    };

    if !homecloud_auth::verify_password(request.password, password_hash).await {
        state.login_attempts().record_failure(&display_name);
        return Err(invalid());
    }

    state.login_attempts().record_success(&display_name);
    issue_session(&state, UserId::from_uuid(user_id), display_name).await
}

/// `POST /api/v1/auth/logout` — always succeeds, so a stale cookie can
/// always be cleared.
pub async fn logout(State(state): State<AppState>, parts: Parts) -> Response {
    if let Some(token) = parts
        .headers
        .get(header::COOKIE)
        .and_then(|value| value.to_str().ok())
        .and_then(|header| cookie_value(header, SESSION_COOKIE))
    {
        if let Err(error) = session::revoke(state.db(), token).await {
            tracing::warn!(error = %error, "session revocation failed");
        }
    }

    let mut response = Json(SessionResponse {
        authenticated: false,
        user_id: None,
        display_name: None,
    })
    .into_response();
    response
        .headers_mut()
        .insert(header::SET_COOKIE, clearing_cookie(state.secure_cookies()));

    response
}

/// `GET /api/v1/session` — who the caller is, if anyone. Answering
/// "nobody" is a normal answer, not an error, so the web app can decide
/// what to render without treating 401 as a failure.
pub async fn session_status(
    State(state): State<AppState>,
    parts: Parts,
) -> Result<Json<SessionResponse>, ApiError> {
    let anonymous = SessionResponse {
        authenticated: false,
        user_id: None,
        display_name: None,
    };

    let Some(token) = parts
        .headers
        .get(header::COOKIE)
        .and_then(|value| value.to_str().ok())
        .and_then(|header| cookie_value(header, SESSION_COOKIE))
    else {
        return Ok(Json(anonymous));
    };

    let session = session::authenticate(state.db(), token)
        .await
        .map_err(|error| {
            tracing::warn!(error = %error, "session lookup failed");
            ApiError::dependency_unavailable("database")
        })?;

    let Some(session) = session else {
        return Ok(Json(anonymous));
    };

    let display_name: Option<String> =
        sqlx::query_scalar("SELECT display_name FROM users WHERE id = $1")
            .bind(session.user.as_uuid())
            .fetch_optional(state.db())
            .await
            .map_err(|error| {
                tracing::warn!(error = %error, "user lookup failed");
                ApiError::dependency_unavailable("database")
            })?;

    Ok(Json(SessionResponse {
        authenticated: true,
        user_id: Some(session.user.to_string()),
        display_name,
    }))
}

pub(crate) async fn issue_session(
    state: &AppState,
    user: UserId,
    display_name: String,
) -> Result<Response, ApiError> {
    let token = session::create(state.db(), user).await.map_err(|error| {
        tracing::error!(error = %error, "session creation failed");
        ApiError::internal()
    })?;

    let mut response = Json(SessionResponse {
        authenticated: true,
        user_id: Some(user.to_string()),
        display_name: Some(display_name),
    })
    .into_response();

    match session_cookie(token.expose(), state.secure_cookies()) {
        Some(cookie) => {
            response.headers_mut().insert(header::SET_COOKIE, cookie);
            *response.status_mut() = StatusCode::OK;
            Ok(response)
        }
        None => {
            tracing::error!("generated session token is not a valid cookie value");
            Err(ApiError::internal())
        }
    }
}

/// Whether a database error is a unique-constraint violation, which is
/// how a duplicate account name arrives.
pub(crate) fn is_unique_violation(error: &sqlx::Error) -> bool {
    matches!(
        error.as_database_error().and_then(|error| error.code()),
        Some(code) if code == "23505"
    )
}

/// A real Argon2 hash of a value nobody knows, used to keep the timing of
/// a failed login independent of whether the account exists.
fn dummy_hash() -> String {
    // Generated once at startup cost; the value itself is irrelevant.
    "$argon2id$v=19$m=19456,t=2,p=1$c29tZS1maXhlZC1zYWx0$RdescudvJCsgt3ub+b+dWRWJTmaaJObG"
        .to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_session_cookie_out_of_a_header() {
        let header = "theme=dark; homecloud_session=abc123; other=1";

        assert_eq!(cookie_value(header, SESSION_COOKIE), Some("abc123"));
    }

    #[test]
    fn a_missing_cookie_is_none() {
        assert_eq!(cookie_value("theme=dark", SESSION_COOKIE), None);
        assert_eq!(cookie_value("", SESSION_COOKIE), None);
    }

    #[test]
    fn does_not_match_a_cookie_whose_name_merely_ends_with_ours() {
        let header = "not_homecloud_session=abc123";

        assert_eq!(cookie_value(header, SESSION_COOKIE), None);
    }

    #[test]
    fn the_session_cookie_is_http_only_and_same_site() {
        let cookie = session_cookie("token-value", true).expect("valid cookie");
        let rendered = cookie.to_str().expect("ascii");

        assert!(rendered.contains("HttpOnly"));
        assert!(rendered.contains("SameSite=Lax"));
        assert!(rendered.contains("Secure"));
        assert!(rendered.starts_with("homecloud_session=token-value"));
    }

    #[test]
    fn insecure_deployments_do_not_get_a_secure_cookie_they_cannot_send() {
        let cookie = session_cookie("token-value", false).expect("valid cookie");

        assert!(!cookie.to_str().expect("ascii").contains("Secure"));
    }
}
