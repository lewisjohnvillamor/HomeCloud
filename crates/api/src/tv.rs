//! Pairing a television, and the capability it gets.
//!
//! A television has no keyboard: typing a password with a four-direction
//! remote is the kind of thing that makes people give up and cast from
//! their phone instead. So the TV shows a short code, someone already
//! signed in approves it, and the TV receives a credential of its own.
//!
//! That credential is deliberately much narrower than a session. It
//! reads the memories of exactly one library, and it can fetch only
//! items that belong in a photo timeline — a paired screen in a living
//! room cannot be talked into displaying a tax return.

use axum::extract::{Path, Query, State};
use axum::http::HeaderMap;
use axum::response::Response;
use axum::Json;
use homecloud_auth::password;
use homecloud_auth::token::{self, Token};
use homecloud_catalog::repository;
use homecloud_catalog::Item;
use homecloud_domain::identity::{ItemId, LibraryId};
use homecloud_media::thumbnail::{is_thumbnailable, ThumbnailSize};
use serde::{Deserialize, Serialize};
use time::{Duration, OffsetDateTime};

use crate::app::AppState;
use crate::auth::CurrentUser;
use crate::error::ApiError;
use crate::library::{
    authorize, catalog_error, memory_groups, parse_item, parse_library, storage_for, MemoryGroup,
};

/// How long a code on screen stays good for. Long enough to find a
/// phone and walk to it; short enough that a photograph of the screen
/// is not a lasting invitation.
const PAIRING_TTL_MINUTES: i64 = 10;

/// Shape of the code shown on the television: two groups of four, from
/// the alphabet that has no look-alike characters, because this is read
/// across a room and typed on a phone.
const CODE_GROUPS: usize = 2;
const CODE_GROUP_LEN: usize = 4;

/// Longest device name accepted, so a name cannot become a payload.
const MAX_DEVICE_NAME: usize = 64;

#[derive(Debug, Serialize)]
pub struct PairingView {
    /// Shown on the television.
    pub code: String,
    /// The television's own secret, used to collect the result. Not the
    /// code: knowing what is on screen must not be enough to take the
    /// credential the approval produces.
    pub poll_token: String,
    pub expires_at: String,
}

fn rfc3339(value: OffsetDateTime) -> String {
    value
        .format(&time::format_description::well_known::Rfc3339)
        .unwrap_or_default()
}

/// Normalises a code as typed: case and separators are noise.
fn normalise(code: &str) -> String {
    password::normalise_recovery_code(code)
}

/// `POST /api/v1/tv/pairings` — a television asks to be paired.
///
/// Takes no session, by definition: this is what a screen that cannot
/// sign in does first.
pub async fn start(State(state): State<AppState>) -> Result<Json<PairingView>, ApiError> {
    let code = password::generate_code(CODE_GROUPS, CODE_GROUP_LEN).map_err(|_| {
        tracing::error!("no entropy available for a pairing code");
        ApiError::internal()
    })?;
    let poll = Token::generate().map_err(|_| {
        tracing::error!("no entropy available for a pairing token");
        ApiError::internal()
    })?;
    let expires_at = OffsetDateTime::now_utc() + Duration::minutes(PAIRING_TTL_MINUTES);

    // Codes are short, so a collision with a live one is possible even
    // though it is unlikely. The unique index catches it and the
    // television simply asks again.
    sqlx::query("INSERT INTO tv_pairings (code, poll_token_hash, expires_at) VALUES ($1, $2, $3)")
        .bind(normalise(&code))
        .bind(token::hash(poll.expose()))
        .bind(expires_at)
        .execute(state.db())
        .await
        .map_err(|error| {
            tracing::warn!(error = %error, "could not open a pairing");
            ApiError::dependency_unavailable("database")
        })?;

    Ok(Json(PairingView {
        code,
        poll_token: poll.expose().to_owned(),
        expires_at: rfc3339(expires_at),
    }))
}

#[derive(Debug, Serialize)]
pub struct PairingStatusView {
    /// `pending` or `approved`. An unknown or expired pairing is a
    /// "not found" instead, so a television stops waiting.
    pub status: &'static str,
    /// The credential, handed over exactly once.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub library_name: Option<String>,
}

/// `GET /api/v1/tv/pairings/{poll_token}` — has anyone approved it yet?
///
/// The television polls this. The approved token is returned once and
/// the row is marked spent in the same statement, so a second reader —
/// even one holding the same secret — gets nothing.
pub async fn poll(
    State(state): State<AppState>,
    Path(poll_token): Path<String>,
) -> Result<Json<PairingStatusView>, ApiError> {
    if !token::is_plausible(&poll_token) {
        return Err(ApiError::not_found());
    }

    let row: Option<(Option<uuid::Uuid>, Option<OffsetDateTime>)> = sqlx::query_as(
        "SELECT device_id, collected_at FROM tv_pairings
         WHERE poll_token_hash = $1 AND expires_at > now()",
    )
    .bind(token::hash(&poll_token))
    .fetch_optional(state.db())
    .await
    .map_err(|error| {
        tracing::warn!(error = %error, "pairing lookup failed");
        ApiError::dependency_unavailable("database")
    })?;

    let Some((device, collected)) = row else {
        // Unknown and expired look the same: a television that waited
        // too long is told to start again, and nothing else is learned.
        return Err(ApiError::not_found());
    };

    let Some(device) = device else {
        return Ok(Json(PairingStatusView {
            status: "pending",
            token: None,
            library_name: None,
        }));
    };

    if collected.is_some() {
        // Already handed over. Whatever is asking now is not the screen
        // that was approved.
        return Err(ApiError::not_found());
    }

    // The credential is minted here rather than at approval, so it never
    // exists before the screen that will hold it comes to collect it.
    let device_token = Token::generate().map_err(|_| {
        tracing::error!("no entropy available for a device token");
        ApiError::internal()
    })?;

    let claimed = sqlx::query(
        "UPDATE tv_pairings SET collected_at = now()
         WHERE poll_token_hash = $1 AND collected_at IS NULL",
    )
    .bind(token::hash(&poll_token))
    .execute(state.db())
    .await
    .map_err(|error| {
        tracing::error!(error = %error, "could not close a pairing");
        ApiError::internal()
    })?
    .rows_affected();

    if claimed == 0 {
        // Someone else collected it between the read and the write.
        return Err(ApiError::not_found());
    }

    let library_name: Option<String> = sqlx::query_scalar(
        "UPDATE tv_devices SET token_hash = $2
         FROM libraries
         WHERE tv_devices.id = $1 AND libraries.id = tv_devices.library_id
         RETURNING libraries.name",
    )
    .bind(device)
    .bind(token::hash(device_token.expose()))
    .fetch_optional(state.db())
    .await
    .map_err(|error| {
        tracing::error!(error = %error, "could not arm a paired device");
        ApiError::internal()
    })?;

    let Some(library_name) = library_name else {
        // The device was revoked between approval and collection.
        return Err(ApiError::not_found());
    };

    tracing::info!("a television collected its pairing");

    Ok(Json(PairingStatusView {
        status: "approved",
        token: Some(device_token.expose().to_owned()),
        library_name: Some(library_name),
    }))
}

#[derive(Debug, Deserialize)]
pub struct ApproveRequest {
    pub library_id: String,
    /// What to call this screen in the list of paired devices.
    pub name: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct DeviceView {
    pub id: String,
    pub name: String,
    pub created_at: String,
    pub last_seen_at: Option<String>,
}

/// `POST /api/v1/tv/pairings/{code}/approve`
///
/// The deliberate human step: someone who is already signed in says that
/// the code on the television is one they are looking at.
pub async fn approve(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Path(code): Path<String>,
    Json(request): Json<ApproveRequest>,
) -> Result<Json<DeviceView>, ApiError> {
    let library = parse_library(&request.library_id)?;
    authorize(&state, user, library).await?;

    let code = normalise(&code);
    // Guessing a code is guessing at a device someone else is holding,
    // so attempts are throttled the same way sign-in is.
    let throttle_key = format!("tv:{user}");
    if let Err(retry_after) = state.login_attempts().check(&throttle_key) {
        return Err(ApiError::too_many_requests(retry_after));
    }

    let name = request
        .name
        .unwrap_or_default()
        .trim()
        .chars()
        .take(MAX_DEVICE_NAME)
        .collect::<String>();
    let name = if name.is_empty() {
        "Television".to_owned()
    } else {
        name
    };

    let pairing: Option<uuid::Uuid> = sqlx::query_scalar(
        "SELECT id FROM tv_pairings
         WHERE code = $1 AND expires_at > now() AND device_id IS NULL",
    )
    .bind(&code)
    .fetch_optional(state.db())
    .await
    .map_err(|error| {
        tracing::warn!(error = %error, "pairing lookup failed");
        ApiError::dependency_unavailable("database")
    })?;

    let Some(pairing) = pairing else {
        state.login_attempts().record_failure(&throttle_key);
        return Err(ApiError::not_found());
    };

    state.login_attempts().record_success(&throttle_key);

    // A placeholder hash until the screen collects its real token: the
    // column is NOT NULL, and a value nobody holds is the safest thing
    // to put there.
    let placeholder = token::hash(
        Token::generate()
            .map_err(|_| ApiError::internal())?
            .expose(),
    );

    let device: (uuid::Uuid, OffsetDateTime) = sqlx::query_as(
        "INSERT INTO tv_devices (library_id, approved_by, token_hash, name)
         VALUES ($1, $2, $3, $4)
         RETURNING id, created_at",
    )
    .bind(library.as_uuid())
    .bind(user.as_uuid())
    .bind(placeholder)
    .bind(&name)
    .fetch_one(state.db())
    .await
    .map_err(|error| {
        tracing::error!(error = %error, "could not record a paired device");
        ApiError::internal()
    })?;

    // Claim the pairing only if it is still unclaimed, so two people
    // approving the same code do not both attach a device to it.
    let claimed =
        sqlx::query("UPDATE tv_pairings SET device_id = $2 WHERE id = $1 AND device_id IS NULL")
            .bind(pairing)
            .bind(device.0)
            .execute(state.db())
            .await
            .map_err(|error| {
                tracing::error!(error = %error, "could not attach a paired device");
                ApiError::internal()
            })?
            .rows_affected();

    if claimed == 0 {
        sqlx::query("DELETE FROM tv_devices WHERE id = $1")
            .bind(device.0)
            .execute(state.db())
            .await
            .ok();

        return Err(ApiError::conflict("That code has already been approved."));
    }

    tracing::info!("a television was approved for a library");

    Ok(Json(DeviceView {
        id: device.0.to_string(),
        name,
        created_at: rfc3339(device.1),
        last_seen_at: None,
    }))
}

/// `GET /api/v1/libraries/{library}/tv` — screens paired with a library.
pub async fn list(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Path(library): Path<String>,
) -> Result<Json<Vec<DeviceView>>, ApiError> {
    let library = parse_library(&library)?;
    authorize(&state, user, library).await?;

    let rows: Vec<(uuid::Uuid, String, OffsetDateTime, Option<OffsetDateTime>)> = sqlx::query_as(
        "SELECT id, name, created_at, last_seen_at FROM tv_devices
         WHERE library_id = $1 AND revoked_at IS NULL
         ORDER BY created_at DESC",
    )
    .bind(library.as_uuid())
    .fetch_all(state.db())
    .await
    .map_err(|error| {
        tracing::warn!(error = %error, "paired device lookup failed");
        ApiError::dependency_unavailable("database")
    })?;

    Ok(Json(
        rows.into_iter()
            .map(|(id, name, created_at, last_seen_at)| DeviceView {
                id: id.to_string(),
                name,
                created_at: rfc3339(created_at),
                last_seen_at: last_seen_at.map(rfc3339),
            })
            .collect(),
    ))
}

/// `DELETE /api/v1/tv/devices/{id}` — unpair a screen.
///
/// Takes effect on the television's next request: there is no cached
/// credential anywhere else.
pub async fn revoke(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Path(device): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let device = uuid::Uuid::parse_str(&device).map_err(|_| ApiError::not_found())?;

    let library: Option<uuid::Uuid> = sqlx::query_scalar(
        "SELECT library_id FROM tv_devices WHERE id = $1 AND revoked_at IS NULL",
    )
    .bind(device)
    .fetch_optional(state.db())
    .await
    .map_err(|error| {
        tracing::warn!(error = %error, "paired device lookup failed");
        ApiError::dependency_unavailable("database")
    })?;

    let Some(library) = library else {
        return Err(ApiError::not_found());
    };

    // Membership is checked before the device is touched, so a device id
    // from another library answers exactly as a made-up one does.
    authorize(&state, user, LibraryId::from_uuid(library)).await?;

    sqlx::query("UPDATE tv_devices SET revoked_at = now() WHERE id = $1")
        .bind(device)
        .execute(state.db())
        .await
        .map_err(|error| {
            tracing::error!(error = %error, "could not unpair a television");
            ApiError::internal()
        })?;

    tracing::info!("a television was unpaired");

    Ok(Json(serde_json::json!({ "revoked": true })))
}

// --- What a paired television may read. No session; the token is the
// whole credential, and it is narrower than one. ---

/// A television's capability: one library, pictures only.
struct Screen {
    library: LibraryId,
}

#[derive(Debug, Deserialize)]
pub struct ScreenQuery {
    /// The device token. A query parameter rather than a header because
    /// an `<img>` on the photo wall cannot send one; the request log
    /// records paths, never query strings.
    pub token: String,
    /// Which item to fetch, for the content and thumbnail routes.
    pub item: Option<String>,
}

/// Resolves a device token, refusing a revoked or unknown one
/// identically.
async fn screen(state: &AppState, device_token: &str) -> Result<Screen, ApiError> {
    if !token::is_plausible(device_token) {
        return Err(ApiError::unauthorized("This screen is not paired."));
    }

    let library: Option<uuid::Uuid> = sqlx::query_scalar(
        "UPDATE tv_devices SET last_seen_at = now()
         WHERE token_hash = $1 AND revoked_at IS NULL
         RETURNING library_id",
    )
    .bind(token::hash(device_token))
    .fetch_optional(state.db())
    .await
    .map_err(|error| {
        tracing::warn!(error = %error, "paired device lookup failed");
        ApiError::dependency_unavailable("database")
    })?;

    match library {
        Some(library) => Ok(Screen {
            library: LibraryId::from_uuid(library),
        }),
        None => Err(ApiError::unauthorized("This screen is not paired.")),
    }
}

/// Loads an item a screen is allowed to show.
///
/// The two rules that make this credential narrow are both here: the
/// item must belong to the paired library, and it must be something that
/// belongs in a photo timeline. A document in the same library is a
/// "not found", exactly as an item from another library is.
async fn picture(state: &AppState, screen: &Screen, item: Option<&str>) -> Result<Item, ApiError> {
    let Some(item) = item else {
        return Err(ApiError::bad_request("No item was requested."));
    };
    let item: ItemId = parse_item(item)?;

    let item = repository::item_in_library(state.db(), screen.library, item)
        .await
        .map_err(catalog_error)?;

    if !item.is_visual_media() || item.trashed_at.is_some() || item.missing_since.is_some() {
        return Err(ApiError::not_found());
    }

    Ok(item)
}

/// `GET /api/v1/tv/memories?token=` — what the wall shows.
pub async fn memories(
    State(state): State<AppState>,
    Query(query): Query<ScreenQuery>,
) -> Result<Json<Vec<MemoryGroup>>, ApiError> {
    let screen = screen(&state, &query.token).await?;

    Ok(Json(memory_groups(&state, screen.library).await?))
}

/// `GET /api/v1/tv/thumbnail?token=&item=`
pub async fn thumbnail(
    State(state): State<AppState>,
    Query(query): Query<ScreenQuery>,
) -> Result<Response, ApiError> {
    let screen = screen(&state, &query.token).await?;
    let item = picture(&state, &screen, query.item.as_deref()).await?;

    if !is_thumbnailable(item.content_type.as_deref()) {
        return Err(ApiError::bad_request(
            "This item does not have a picture preview.",
        ));
    }

    let storage = storage_for(&state, screen.library).await?;

    crate::thumbnails::render(&storage, &item, ThumbnailSize::Large).await
}

/// `GET /api/v1/tv/content?token=&item=` — the full picture, for the
/// slideshow.
pub async fn content(
    State(state): State<AppState>,
    Query(query): Query<ScreenQuery>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    let screen = screen(&state, &query.token).await?;
    let item = picture(&state, &screen, query.item.as_deref()).await?;
    let storage = storage_for(&state, screen.library).await?;

    crate::transfers::stream_file(&storage, &item, &headers).await
}

/// Removes pairings nobody used. Correctness does not depend on it —
/// expiry is enforced on every lookup — but a code table should not grow
/// forever.
pub async fn purge_expired(pool: &sqlx::PgPool) -> Result<u64, sqlx::Error> {
    let result = sqlx::query("DELETE FROM tv_pairings WHERE expires_at <= now()")
        .execute(pool)
        .await?;

    Ok(result.rows_affected())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_code_is_read_the_way_a_person_types_it() {
        assert_eq!(normalise("abcd-efgh"), "ABCDEFGH");
        assert_eq!(normalise(" ABCD EFGH "), "ABCDEFGH");
    }
}
