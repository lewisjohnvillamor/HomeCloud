//! The private AI switch.
//!
//! Off by default and off unless someone turns it on. Two questions are
//! kept apart on purpose: what the owner asked for, and what this
//! machine can actually do. Conflating them would let the interface
//! accept a setting and quietly do nothing, which is the failure mode
//! this whole feature has to avoid.
//!
//! Only the owner changes it. Face grouping in particular is required to
//! be an explicit choice rather than something that arrives with an
//! upgrade, and the same reasoning covers the rest: enabling this
//! commits someone else's machine to work.

use axum::extract::{Path, State};
use axum::Json;
use homecloud_ai::{Capabilities, Profile};
use homecloud_domain::identity::{LibraryId, UserId};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;

use crate::app::AppState;
use crate::auth::CurrentUser;
use crate::error::ApiError;
use crate::library::{authorize, parse_library};

#[derive(Debug, Serialize)]
pub struct AiSettingsView {
    /// What the owner asked for.
    pub profile: &'static str,
    /// What this machine can do, whatever was asked for.
    pub ocr_available: bool,
    /// The most this deployment can honour, so the interface can say
    /// plainly that a choice will not take effect yet.
    pub supported_profile: &'static str,
    /// How many files are waiting to be read, so enabling this is not a
    /// silent commitment of somebody's evening.
    pub pending_items: i64,
}

#[derive(Debug, Deserialize)]
pub struct AiSettingsRequest {
    pub profile: String,
}

/// `GET /api/v1/libraries/{library}/ai`
pub async fn read(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Path(library): Path<String>,
) -> Result<Json<AiSettingsView>, ApiError> {
    let library = parse_library(&library)?;
    authorize(&state, user, library).await?;

    let profile = profile_of(state.db(), library).await?;
    let capabilities = Capabilities::detect().await;

    Ok(Json(AiSettingsView {
        profile: profile.as_str(),
        ocr_available: capabilities.ocr,
        supported_profile: capabilities.supported().as_str(),
        pending_items: pending_count(state.db(), library, profile).await,
    }))
}

/// `PUT /api/v1/libraries/{library}/ai`
pub async fn update(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Path(library): Path<String>,
    Json(request): Json<AiSettingsRequest>,
) -> Result<Json<AiSettingsView>, ApiError> {
    let library = parse_library(&library)?;
    require_owner(&state, user, library).await?;

    let Some(profile) = Profile::parse(request.profile.trim()) else {
        return Err(ApiError::bad_request(
            "Choose one of: off, text, photos, people.",
        ));
    };

    sqlx::query(
        "INSERT INTO ai_settings (library_id, profile, updated_by)
         VALUES ($1, $2, $3)
         ON CONFLICT (library_id)
         DO UPDATE SET profile = $2, updated_by = $3, updated_at = now()",
    )
    .bind(library.as_uuid())
    .bind(profile.as_str())
    .bind(user.as_uuid())
    .execute(state.db())
    .await
    .map_err(|error| {
        tracing::error!(error = %error, "could not save the AI setting");
        ApiError::internal()
    })?;

    // Turning it off takes the derived text with it. Everything AI wrote
    // is derived: dropping it costs a rescan and nothing else, and
    // leaving it behind after someone said no would be the wrong answer
    // to the only question they asked.
    if profile.is_off() {
        let removed = forget_derived(state.db(), library).await?;
        tracing::info!(removed, "private AI turned off and its text removed");
    } else {
        tracing::info!(profile = profile.as_str(), "private AI turned on");
    }

    let capabilities = Capabilities::detect().await;

    Ok(Json(AiSettingsView {
        profile: profile.as_str(),
        ocr_available: capabilities.ocr,
        supported_profile: capabilities.supported().as_str(),
        pending_items: pending_count(state.db(), library, profile).await,
    }))
}

/// What a library has turned on. Absent means off.
pub async fn profile_of(pool: &PgPool, library: LibraryId) -> Result<Profile, ApiError> {
    let stored: Option<String> =
        sqlx::query_scalar("SELECT profile FROM ai_settings WHERE library_id = $1")
            .bind(library.as_uuid())
            .fetch_optional(pool)
            .await
            .map_err(|error| {
                tracing::warn!(error = %error, "could not read the AI setting");
                ApiError::dependency_unavailable("database")
            })?;

    Ok(stored
        .as_deref()
        .and_then(Profile::parse)
        .unwrap_or_default())
}

/// Removes everything AI wrote for a library, leaving text read straight
/// out of files alone.
pub async fn forget_derived(pool: &PgPool, library: LibraryId) -> Result<u64, ApiError> {
    let removed = sqlx::query("DELETE FROM item_text WHERE library_id = $1 AND source = 'ocr'")
        .bind(library.as_uuid())
        .execute(pool)
        .await
        .map_err(|error| {
            tracing::error!(error = %error, "could not remove AI-derived text");
            ApiError::internal()
        })?
        .rows_affected();

    Ok(removed)
}

/// How much work turning this on would mean. Best effort: a number the
/// interface shows, never something a request depends on.
async fn pending_count(pool: &PgPool, library: LibraryId, profile: Profile) -> i64 {
    if !profile.includes_ocr() {
        return 0;
    }

    sqlx::query_scalar::<_, i64>(
        "SELECT count(*) FROM items i
         LEFT JOIN item_text t ON t.item_id = i.id
         WHERE i.library_id = $1
           AND i.kind = 'file'
           AND i.trashed_at IS NULL
           AND i.missing_since IS NULL
           AND i.content_type LIKE 'image/%'
           AND (t.item_id IS NULL OR t.source <> 'ocr')",
    )
    .bind(library.as_uuid())
    .fetch_one(pool)
    .await
    .unwrap_or(0)
}

/// Enabling this commits the machine to work and, for faces, to grouping
/// pictures of people. Both are the owner's call.
async fn require_owner(state: &AppState, user: UserId, library: LibraryId) -> Result<(), ApiError> {
    let role: Option<String> = sqlx::query_scalar(
        "SELECT role FROM library_members WHERE library_id = $1 AND user_id = $2",
    )
    .bind(library.as_uuid())
    .bind(user.as_uuid())
    .fetch_optional(state.db())
    .await
    .map_err(|error| {
        tracing::warn!(error = %error, "membership lookup failed");
        ApiError::dependency_unavailable("database")
    })?;

    match role.as_deref() {
        Some("owner") => Ok(()),
        Some(_) => Err(ApiError::forbidden(
            "Only the library owner can turn private AI on or off.",
        )),
        None => Err(ApiError::not_found()),
    }
}
