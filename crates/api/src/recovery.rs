//! Account recovery.
//!
//! A server in someone's house has no support desk. If they forget their
//! password, the alternative to a recovery code is editing the database
//! by hand, so the code is generated for them whether they ask or not
//! and shown once at setup.
//!
//! Deliberately no email: this deployment may have no way to send any,
//! and making the most fragile flow depend on the most fragile
//! infrastructure is how people lose access to their own files.

use axum::extract::State;
use axum::response::Response;
use axum::Json;
use homecloud_auth::password;
use homecloud_domain::identity::UserId;
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use crate::app::AppState;
use crate::auth::{self, CurrentUser};
use crate::error::ApiError;

#[derive(Debug, Serialize)]
pub struct RecoveryCodeView {
    /// Shown once, at the moment it is created. Never returned again.
    pub code: String,
}

#[derive(Debug, Serialize)]
pub struct RecoveryStatusView {
    pub has_code: bool,
    pub created_at: Option<String>,
}

/// Generates a code for a user and stores only its hash.
pub async fn issue_code(state: &AppState, user: UserId) -> Result<String, ApiError> {
    let code = password::generate_recovery_code().map_err(|_| {
        tracing::error!("no entropy available for a recovery code");
        ApiError::internal()
    })?;

    let hash = homecloud_auth::hash_recovery_code(code.clone())
        .await
        .map_err(|_| ApiError::internal())?;

    sqlx::query(
        "UPDATE users SET recovery_code_hash = $2, recovery_code_set_at = now() WHERE id = $1",
    )
    .bind(user.as_uuid())
    .bind(&hash)
    .execute(state.db())
    .await
    .map_err(|error| {
        tracing::error!(error = %error, "could not store a recovery code");
        ApiError::internal()
    })?;

    Ok(code)
}

/// `GET /api/v1/auth/recovery` — whether this account has a code, never
/// the code itself.
pub async fn status(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
) -> Result<Json<RecoveryStatusView>, ApiError> {
    let row: Option<Option<OffsetDateTime>> = sqlx::query_scalar(
        "SELECT recovery_code_set_at FROM users WHERE id = $1 AND recovery_code_hash IS NOT NULL",
    )
    .bind(user.as_uuid())
    .fetch_optional(state.db())
    .await
    .map_err(|error| {
        tracing::warn!(error = %error, "recovery status lookup failed");
        ApiError::dependency_unavailable("database")
    })?;

    let created_at = row.flatten();

    Ok(Json(RecoveryStatusView {
        has_code: created_at.is_some(),
        created_at: created_at.map(|value| {
            value
                .format(&time::format_description::well_known::Rfc3339)
                .unwrap_or_default()
        }),
    }))
}

/// `POST /api/v1/auth/recovery` — replaces any existing code.
///
/// Regenerating invalidates the old one, which is the point: a code
/// written on paper that has been seen by someone else should be
/// replaceable in one action.
pub async fn regenerate(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
) -> Result<Json<RecoveryCodeView>, ApiError> {
    let code = issue_code(&state, user).await?;

    tracing::info!("a recovery code was regenerated");

    Ok(Json(RecoveryCodeView { code }))
}

#[derive(Debug, Deserialize)]
pub struct RecoverRequest {
    pub display_name: String,
    pub recovery_code: String,
    pub new_password: String,
}

/// `POST /api/v1/auth/recover` — takes no session, by definition.
///
/// A correct code sets a new password, ends every existing session, and
/// burns itself. A new code is issued in the same response, because an
/// account with no way back in is exactly the state this feature exists
/// to prevent.
pub async fn recover(
    State(state): State<AppState>,
    Json(request): Json<RecoverRequest>,
) -> Result<Response, ApiError> {
    let display_name = request.display_name.trim().to_owned();

    // Recovery is password guessing by another name, so it shares the
    // sign-in throttle rather than offering an unlimited side door.
    if let Err(retry_after) = state.login_attempts().check(&display_name) {
        return Err(ApiError::too_many_requests(retry_after));
    }

    let refused = || ApiError::unauthorized("That name and recovery code do not match.");

    password::check_policy(&request.new_password)
        .map_err(|error| ApiError::bad_request(error.to_string()))?;

    let row: Option<(uuid::Uuid, Option<String>)> = sqlx::query_as(
        "SELECT id, recovery_code_hash FROM users WHERE lower(display_name) = lower($1)",
    )
    .bind(&display_name)
    .fetch_optional(state.db())
    .await
    .map_err(|error| {
        tracing::warn!(error = %error, "recovery lookup failed");
        ApiError::dependency_unavailable("database")
    })?;

    let Some((user_id, Some(stored_hash))) = row else {
        // Spend the same time either way: whether an account exists, and
        // whether it has a code, must not be readable from the clock.
        homecloud_auth::verify_recovery_code(request.recovery_code, dummy_hash()).await;
        state.login_attempts().record_failure(&display_name);
        return Err(refused());
    };

    if !homecloud_auth::verify_recovery_code(request.recovery_code, stored_hash).await {
        state.login_attempts().record_failure(&display_name);
        return Err(refused());
    }

    let user = UserId::from_uuid(user_id);
    let password_hash = homecloud_auth::hash_password(request.new_password)
        .await
        .map_err(|_| ApiError::internal())?;

    // The old code is cleared in the same statement that sets the new
    // password, so a code cannot be used twice even under a race.
    let updated = sqlx::query(
        "UPDATE users
         SET password_hash = $2, recovery_code_hash = NULL, recovery_code_set_at = NULL
         WHERE id = $1 AND recovery_code_hash IS NOT NULL",
    )
    .bind(user.as_uuid())
    .bind(&password_hash)
    .execute(state.db())
    .await
    .map_err(|error| {
        tracing::error!(error = %error, "could not apply a recovery");
        ApiError::internal()
    })?
    .rows_affected();

    if updated == 0 {
        state.login_attempts().record_failure(&display_name);
        return Err(refused());
    }

    // Whoever knew the old password loses their sessions: recovery is
    // also what someone does after a compromise.
    if let Err(error) = homecloud_auth::session::revoke_all_for_user(state.db(), user).await {
        tracing::warn!(error = %error, "could not end sessions after recovery");
    }

    state.login_attempts().record_success(&display_name);
    tracing::info!("an account was recovered with its recovery code");

    // Issue a fresh code immediately, carried once in this response: an
    // account with no way back in is the state this feature exists to
    // prevent.
    let next_code = issue_code(&state, user).await?;

    auth::issue_session_with(
        &state,
        user,
        display_name,
        Some(serde_json::json!({ "recovery_code": next_code })),
    )
    .await
}

/// A real Argon2 hash of a value nobody knows, so a failed recovery
/// costs the same time whether or not the account exists.
fn dummy_hash() -> String {
    "$argon2id$v=19$m=19456,t=2,p=1$c29tZS1maXhlZC1zYWx0$RdescudvJCsgt3ub+b+dWRWJTmaaJObG"
        .to_owned()
}
