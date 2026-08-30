//! Passkey routes.
//!
//! Registration and sign-in are two-step ceremonies: the server issues a
//! challenge, the authenticator answers it, and the server verifies the
//! answer against the challenge it issued. The intermediate state is
//! held in memory with a short expiry — it is worthless after a few
//! minutes, and losing it on restart costs a retry.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::Response;
use axum::Json;
use homecloud_auth::passkey::{self, PasskeyError};
use homecloud_domain::identity::UserId;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use webauthn_rs::prelude::{
    PasskeyAuthentication, PasskeyRegistration, PublicKeyCredential, RegisterPublicKeyCredential,
};

use crate::app::AppState;
use crate::auth::{self, CurrentUser};
use crate::error::ApiError;

/// How long a challenge stays valid. Long enough to find a security key
/// in a drawer, short enough that an abandoned ceremony does not linger.
const CEREMONY_TTL: Duration = Duration::from_secs(5 * 60);

/// Most ceremonies held at once, so a flood of started-and-abandoned
/// registrations cannot grow this map without limit.
const MAX_CEREMONIES: usize = 256;

/// In-flight challenges.
#[derive(Debug, Default)]
pub struct Ceremonies {
    registrations: Mutex<HashMap<Uuid, (UserId, PasskeyRegistration, Instant)>>,
    authentications: Mutex<HashMap<Uuid, (UserId, PasskeyAuthentication, Instant)>>,
}

impl Ceremonies {
    pub fn new() -> Self {
        Self::default()
    }

    fn store_registration(&self, user: UserId, state: PasskeyRegistration) -> Uuid {
        let id = Uuid::new_v4();
        let mut registrations = lock(&self.registrations);

        registrations.retain(|_, (_, _, started)| started.elapsed() < CEREMONY_TTL);
        if registrations.len() >= MAX_CEREMONIES {
            registrations.clear();
        }
        registrations.insert(id, (user, state, Instant::now()));

        id
    }

    fn take_registration(&self, id: Uuid, user: UserId) -> Option<PasskeyRegistration> {
        let (owner, state, started) = lock(&self.registrations).remove(&id)?;

        // The ceremony belongs to whoever started it: a challenge issued
        // for one account must not complete for another.
        (owner == user && started.elapsed() < CEREMONY_TTL).then_some(state)
    }

    fn store_authentication(&self, user: UserId, state: PasskeyAuthentication) -> Uuid {
        let id = Uuid::new_v4();
        let mut authentications = lock(&self.authentications);

        authentications.retain(|_, (_, _, started)| started.elapsed() < CEREMONY_TTL);
        if authentications.len() >= MAX_CEREMONIES {
            authentications.clear();
        }
        authentications.insert(id, (user, state, Instant::now()));

        id
    }

    fn take_authentication(&self, id: Uuid) -> Option<(UserId, PasskeyAuthentication)> {
        let (user, state, started) = lock(&self.authentications).remove(&id)?;

        (started.elapsed() < CEREMONY_TTL).then_some((user, state))
    }
}

fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|poisoned| {
        tracing::warn!("a passkey ceremony lock was poisoned; continuing");
        poisoned.into_inner()
    })
}

#[derive(Debug, Serialize)]
pub struct ChallengeResponse {
    /// Identifies the challenge when the browser sends its answer back.
    pub ceremony_id: String,
    /// The WebAuthn options, passed to the browser unchanged.
    pub options: serde_json::Value,
}

#[derive(Debug, Serialize)]
pub struct PasskeyView {
    pub id: String,
    pub nickname: String,
    pub created_at: String,
    pub last_used_at: Option<String>,
}

fn rfc3339(value: time::OffsetDateTime) -> String {
    value
        .format(&time::format_description::well_known::Rfc3339)
        .unwrap_or_default()
}

/// `POST /api/v1/auth/passkeys/register/options`
pub async fn register_options(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
) -> Result<Json<ChallengeResponse>, ApiError> {
    let service = state.passkeys().ok_or_else(not_configured)?;

    let display_name: String = sqlx::query_scalar("SELECT display_name FROM users WHERE id = $1")
        .bind(user.as_uuid())
        .fetch_one(state.db())
        .await
        .map_err(|error| {
            tracing::warn!(error = %error, "user lookup failed");
            ApiError::dependency_unavailable("database")
        })?;

    let (options, ceremony) = service
        .start_registration(state.db(), user, &display_name)
        .await
        .map_err(passkey_error)?;

    Ok(Json(ChallengeResponse {
        ceremony_id: state
            .ceremonies()
            .store_registration(user, ceremony)
            .to_string(),
        options,
    }))
}

#[derive(Debug, Deserialize)]
pub struct RegisterVerifyRequest {
    pub ceremony_id: String,
    pub nickname: Option<String>,
    pub credential: RegisterPublicKeyCredential,
}

/// `POST /api/v1/auth/passkeys/register/verify`
pub async fn register_verify(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Json(request): Json<RegisterVerifyRequest>,
) -> Result<Json<PasskeyView>, ApiError> {
    let service = state.passkeys().ok_or_else(not_configured)?;

    let ceremony_id = Uuid::parse_str(&request.ceremony_id).map_err(|_| challenge_expired())?;
    let ceremony = state
        .ceremonies()
        .take_registration(ceremony_id, user)
        .ok_or_else(challenge_expired)?;

    let nickname = request
        .nickname
        .as_deref()
        .map(str::trim)
        .filter(|nickname| !nickname.is_empty() && nickname.chars().count() <= 64)
        .unwrap_or("This device")
        .to_owned();

    let id = service
        .finish_registration(state.db(), user, &nickname, request.credential, &ceremony)
        .await
        .map_err(passkey_error)?;

    tracing::info!("a passkey was registered");

    Ok(Json(PasskeyView {
        id: id.to_string(),
        nickname,
        created_at: rfc3339(time::OffsetDateTime::now_utc()),
        last_used_at: None,
    }))
}

#[derive(Debug, Deserialize)]
pub struct LoginOptionsRequest {
    pub display_name: String,
}

/// `POST /api/v1/auth/passkeys/login/options`
///
/// Needs the account name because this deployment stores passkeys per
/// account rather than as discoverable credentials; the browser is told
/// which credentials may answer.
pub async fn login_options(
    State(state): State<AppState>,
    Json(request): Json<LoginOptionsRequest>,
) -> Result<Json<ChallengeResponse>, ApiError> {
    let service = state.passkeys().ok_or_else(not_configured)?;
    let display_name = request.display_name.trim().to_owned();

    if let Err(retry_after) = state.login_attempts().check(&display_name) {
        return Err(ApiError::too_many_requests(retry_after));
    }

    let user: Option<Uuid> =
        sqlx::query_scalar("SELECT id FROM users WHERE lower(display_name) = lower($1)")
            .bind(&display_name)
            .fetch_optional(state.db())
            .await
            .map_err(|error| {
                tracing::warn!(error = %error, "user lookup failed");
                ApiError::dependency_unavailable("database")
            })?;

    // One answer for "no such account" and "that account has no
    // passkey": the sign-in screen must not become an account directory.
    let refused = || ApiError::unauthorized("No passkey is available for that name.");

    let user = UserId::from_uuid(user.ok_or_else(refused)?);
    let (options, ceremony) = match service.start_authentication(state.db(), user).await {
        Ok(started) => started,
        Err(PasskeyError::NoCredentials) => {
            state.login_attempts().record_failure(&display_name);
            return Err(refused());
        }
        Err(error) => return Err(passkey_error(error)),
    };

    Ok(Json(ChallengeResponse {
        ceremony_id: state
            .ceremonies()
            .store_authentication(user, ceremony)
            .to_string(),
        options,
    }))
}

#[derive(Debug, Deserialize)]
pub struct LoginVerifyRequest {
    pub ceremony_id: String,
    pub credential: PublicKeyCredential,
}

/// `POST /api/v1/auth/passkeys/login/verify`
pub async fn login_verify(
    State(state): State<AppState>,
    Json(request): Json<LoginVerifyRequest>,
) -> Result<Response, ApiError> {
    let service = state.passkeys().ok_or_else(not_configured)?;

    let ceremony_id = Uuid::parse_str(&request.ceremony_id).map_err(|_| challenge_expired())?;
    let (user, ceremony) = state
        .ceremonies()
        .take_authentication(ceremony_id)
        .ok_or_else(challenge_expired)?;

    service
        .finish_authentication(state.db(), user, request.credential, &ceremony)
        .await
        .map_err(|error| match error {
            PasskeyError::Rejected => ApiError::unauthorized("That passkey was not accepted."),
            other => passkey_error(other),
        })?;

    let display_name: String = sqlx::query_scalar("SELECT display_name FROM users WHERE id = $1")
        .bind(user.as_uuid())
        .fetch_one(state.db())
        .await
        .map_err(|error| {
            tracing::warn!(error = %error, "user lookup failed");
            ApiError::dependency_unavailable("database")
        })?;

    state.login_attempts().record_success(&display_name);
    tracing::info!("a passkey sign-in succeeded");

    auth::issue_session(&state, user, display_name).await
}

/// `GET /api/v1/auth/passkeys`
pub async fn list(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
) -> Result<Json<Vec<PasskeyView>>, ApiError> {
    let passkeys = passkey::list_for_user(state.db(), user)
        .await
        .map_err(passkey_error)?;

    Ok(Json(
        passkeys
            .into_iter()
            .map(|passkey| PasskeyView {
                id: passkey.id.to_string(),
                nickname: passkey.nickname,
                created_at: rfc3339(passkey.created_at),
                last_used_at: passkey.last_used_at.map(rfc3339),
            })
            .collect(),
    ))
}

/// `DELETE /api/v1/auth/passkeys/{credential}`
pub async fn remove(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Path(credential): Path<String>,
) -> Result<StatusCode, ApiError> {
    let credential = Uuid::parse_str(&credential).map_err(|_| ApiError::not_found())?;

    let removed = passkey::remove(state.db(), user, credential)
        .await
        .map_err(passkey_error)?;

    if removed {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(ApiError::not_found())
    }
}

/// Whether this server can do passkeys at all, so the UI can offer them
/// only when they will work.
pub fn is_available(state: &AppState) -> bool {
    state.passkeys().is_some()
}

fn not_configured() -> ApiError {
    ApiError::conflict(
        "Passkeys need this server's public address to be configured. Set HOMECLOUD_PUBLIC_ORIGIN.",
    )
}

fn challenge_expired() -> ApiError {
    ApiError::bad_request("That sign-in attempt expired. Try again.")
}

fn passkey_error(error: PasskeyError) -> ApiError {
    match error {
        PasskeyError::NotConfigured => not_configured(),
        PasskeyError::Rejected => ApiError::bad_request("That passkey could not be verified."),
        PasskeyError::AlreadyRegistered => {
            ApiError::conflict("That passkey is already registered.")
        }
        PasskeyError::NoCredentials => {
            ApiError::unauthorized("No passkey is available for that name.")
        }
        PasskeyError::Database(error) => {
            tracing::warn!(error = %error, "a passkey query failed");
            ApiError::dependency_unavailable("database")
        }
    }
}
