//! Upload request links.
//!
//! The mirror image of a share. A share lets someone read one thing; an
//! upload request lets someone write into one folder without seeing what
//! is already in it. "Send me the wedding photos" is the case, and the
//! person sending them should not need an account.
//!
//! This is the only capability in the product that lets an
//! unauthenticated stranger write, so it is the most tightly bounded.
//! A link names one folder and nothing else, cannot read anything at
//! all, carries its own limits on how many files and how many bytes it
//! will ever accept, and names files itself rather than taking a path
//! from whoever is uploading.

use axum::body::Body;
use axum::extract::{Path, Query, State};
use axum::Json;
use futures::StreamExt;
use homecloud_auth::token::{self, Token};
use homecloud_catalog::repository;
use homecloud_domain::identity::{ItemId, LibraryId};
use homecloud_storage::{LibraryPath, MutableStorage};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use time::{Duration, OffsetDateTime};

use crate::app::AppState;
use crate::auth::CurrentUser;
use crate::error::ApiError;
use crate::items::{record_uploaded_file, storage_error};
use crate::library::{authorize, catalog_error, parse_item, parse_library, storage_for};
use crate::view::ItemView;

/// Longest expiry a request link may carry.
const MAX_EXPIRY_DAYS: i64 = 365;

/// Defaults, deliberately modest. A link that lets a stranger write is
/// not the place for generous limits, and an owner can make another one.
const DEFAULT_MAX_FILES: i32 = 50;
const DEFAULT_MAX_BYTES: i64 = 2 * 1024 * 1024 * 1024;

/// Ceilings an owner cannot raise past.
const LIMIT_MAX_FILES: i32 = 500;
const LIMIT_MAX_BYTES: i64 = 20 * 1024 * 1024 * 1024;

/// Longest title accepted, so it cannot become a payload.
const MAX_TITLE: usize = 96;

/// Longest name accepted from an uploader, before it is sanitised.
const MAX_NAME: usize = 200;

/// One row of the owner's list, as the database returns it.
type LinkRow = (
    uuid::Uuid,
    uuid::Uuid,
    String,
    String,
    OffsetDateTime,
    Option<OffsetDateTime>,
    i32,
    i64,
    i32,
    i64,
);

/// One resolved link, as the database returns it.
type CapabilityRow = (
    uuid::Uuid,
    uuid::Uuid,
    uuid::Uuid,
    String,
    String,
    i32,
    i64,
    i32,
    i64,
);

fn rfc3339(value: OffsetDateTime) -> String {
    value
        .format(&time::format_description::well_known::Rfc3339)
        .unwrap_or_default()
}

#[derive(Debug, Deserialize)]
pub struct CreateRequest {
    /// What to call this, for whoever opens the link.
    pub title: Option<String>,
    pub expires_in_days: Option<i64>,
    pub max_files: Option<i32>,
    pub max_bytes: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct RequestView {
    pub id: String,
    pub item_id: String,
    pub folder_name: String,
    pub title: String,
    pub created_at: String,
    pub expires_at: Option<String>,
    pub max_files: i32,
    pub max_bytes: i64,
    pub received_files: i32,
    pub received_bytes: i64,
    /// Only on the response that creates the link: the token is never
    /// stored and cannot be shown again.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token: Option<String>,
}

/// `POST /api/v1/items/{item}/upload-requests`
pub async fn create(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Path(item): Path<String>,
    Json(request): Json<CreateRequest>,
) -> Result<Json<RequestView>, ApiError> {
    let item_id = parse_item(&item)?;
    let folder = repository::item_for_user(state.db(), user, item_id)
        .await
        .map_err(catalog_error)?;

    // Files have to land somewhere: a link pointed at a file would have
    // nowhere to put anything.
    if !folder.is_folder() {
        return Err(ApiError::bad_request(
            "An upload link points at a folder, not a file.",
        ));
    }
    if folder.trashed_at.is_some() || folder.missing_since.is_some() {
        return Err(ApiError::conflict(
            "Restore this folder before asking for uploads to it.",
        ));
    }

    let expires_at = match request.expires_in_days {
        None => None,
        Some(days) if (1..=MAX_EXPIRY_DAYS).contains(&days) => {
            Some(OffsetDateTime::now_utc() + Duration::days(days))
        }
        Some(_) => {
            return Err(ApiError::bad_request(format!(
                "Choose an expiry between 1 and {MAX_EXPIRY_DAYS} days."
            )))
        }
    };

    let max_files = request.max_files.unwrap_or(DEFAULT_MAX_FILES);
    let max_bytes = request.max_bytes.unwrap_or(DEFAULT_MAX_BYTES);

    if !(1..=LIMIT_MAX_FILES).contains(&max_files) {
        return Err(ApiError::bad_request(format!(
            "A link accepts between 1 and {LIMIT_MAX_FILES} files."
        )));
    }
    if !(1..=LIMIT_MAX_BYTES).contains(&max_bytes) {
        return Err(ApiError::bad_request(
            "That total size is outside what a link may accept.",
        ));
    }

    let title = match request.title.as_deref().map(str::trim) {
        None | Some("") => format!("Send files to {}", folder.name),
        Some(title) if title.chars().count() > MAX_TITLE => {
            return Err(ApiError::bad_request(format!(
                "A title is at most {MAX_TITLE} characters."
            )))
        }
        Some(title) if title.chars().any(char::is_control) => {
            return Err(ApiError::bad_request(
                "That title contains control characters.",
            ))
        }
        Some(title) => title.to_owned(),
    };

    let token = Token::generate().map_err(|_| {
        tracing::error!("no entropy available for an upload request token");
        ApiError::internal()
    })?;

    let row: (uuid::Uuid, OffsetDateTime) = sqlx::query_as(
        "INSERT INTO upload_requests
            (library_id, item_id, created_by, token_hash, title, expires_at, max_files, max_bytes)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
         RETURNING id, created_at",
    )
    .bind(folder.library.as_uuid())
    .bind(folder.id.as_uuid())
    .bind(user.as_uuid())
    .bind(token::hash(token.expose()))
    .bind(&title)
    .bind(expires_at)
    .bind(max_files)
    .bind(max_bytes)
    .fetch_one(state.db())
    .await
    .map_err(|error| {
        tracing::error!(error = %error, "could not create an upload request");
        ApiError::internal()
    })?;

    tracing::info!("an upload request link was created");

    Ok(Json(RequestView {
        id: row.0.to_string(),
        item_id: folder.id.to_string(),
        folder_name: folder.name.clone(),
        title,
        created_at: rfc3339(row.1),
        expires_at: expires_at.map(rfc3339),
        max_files,
        max_bytes,
        received_files: 0,
        received_bytes: 0,
        token: Some(token.expose().to_owned()),
    }))
}

/// `GET /api/v1/libraries/{library}/upload-requests`
pub async fn list(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Path(library): Path<String>,
) -> Result<Json<Vec<RequestView>>, ApiError> {
    let library = parse_library(&library)?;
    authorize(&state, user, library).await?;

    let rows: Vec<LinkRow> = sqlx::query_as(
        "SELECT r.id, r.item_id, i.name, r.title, r.created_at, r.expires_at,
                r.max_files, r.max_bytes, r.received_files, r.received_bytes
         FROM upload_requests r
         JOIN items i ON i.id = r.item_id
         WHERE r.library_id = $1
           AND r.revoked_at IS NULL
           AND (r.expires_at IS NULL OR r.expires_at > now())
         ORDER BY r.created_at DESC",
    )
    .bind(library.as_uuid())
    .fetch_all(state.db())
    .await
    .map_err(|error| {
        tracing::warn!(error = %error, "could not list upload requests");
        ApiError::dependency_unavailable("database")
    })?;

    Ok(Json(
        rows.into_iter()
            .map(|row| RequestView {
                id: row.0.to_string(),
                item_id: row.1.to_string(),
                folder_name: row.2,
                title: row.3,
                created_at: rfc3339(row.4),
                expires_at: row.5.map(rfc3339),
                max_files: row.6,
                max_bytes: row.7,
                received_files: row.8,
                received_bytes: row.9,
                token: None,
            })
            .collect(),
    ))
}

/// `DELETE /api/v1/upload-requests/{id}` — stop accepting files.
pub async fn revoke(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let id = uuid::Uuid::parse_str(&id).map_err(|_| ApiError::not_found())?;

    let library: Option<uuid::Uuid> = sqlx::query_scalar(
        "SELECT library_id FROM upload_requests WHERE id = $1 AND revoked_at IS NULL",
    )
    .bind(id)
    .fetch_optional(state.db())
    .await
    .map_err(|error| {
        tracing::warn!(error = %error, "upload request lookup failed");
        ApiError::dependency_unavailable("database")
    })?;

    let Some(library) = library else {
        return Err(ApiError::not_found());
    };

    authorize(&state, user, LibraryId::from_uuid(library)).await?;

    sqlx::query("UPDATE upload_requests SET revoked_at = now() WHERE id = $1")
        .bind(id)
        .execute(state.db())
        .await
        .map_err(|error| {
            tracing::error!(error = %error, "could not revoke an upload request");
            ApiError::internal()
        })?;

    tracing::info!("an upload request link was revoked");

    Ok(Json(serde_json::json!({ "revoked": true })))
}

// --- What the person holding the link can do. No session. ---

/// A resolved upload request: exactly one folder, and the room left.
struct Capability {
    id: uuid::Uuid,
    library: LibraryId,
    folder: ItemId,
    title: String,
    folder_name: String,
    files_left: i32,
    bytes_left: i64,
}

/// Resolves a token, answering unknown, expired, and revoked links
/// identically so a visitor cannot learn that one once existed.
async fn resolve(state: &AppState, token: &str) -> Result<Capability, ApiError> {
    if !token::is_plausible(token) {
        return Err(ApiError::not_found());
    }

    let row: Option<CapabilityRow> = sqlx::query_as(
        "SELECT r.id, r.library_id, r.item_id, r.title, i.name,
                    r.max_files, r.max_bytes, r.received_files, r.received_bytes
             FROM upload_requests r
             JOIN items i ON i.id = r.item_id
             WHERE r.token_hash = $1
               AND r.revoked_at IS NULL
               AND (r.expires_at IS NULL OR r.expires_at > now())
               AND i.trashed_at IS NULL
               AND i.missing_since IS NULL",
    )
    .bind(token::hash(token))
    .fetch_optional(state.db())
    .await
    .map_err(|error| {
        tracing::warn!(error = %error, "upload request lookup failed");
        ApiError::dependency_unavailable("database")
    })?;

    let Some((id, library, folder, title, folder_name, max_files, max_bytes, files, bytes)) = row
    else {
        return Err(ApiError::not_found());
    };

    Ok(Capability {
        id,
        library: LibraryId::from_uuid(library),
        folder: ItemId::from_uuid(folder),
        title,
        folder_name,
        files_left: max_files - files,
        bytes_left: max_bytes - bytes,
    })
}

#[derive(Debug, Serialize)]
pub struct PublicRequestView {
    pub title: String,
    /// The folder's name, which is what the sender is told they are
    /// sending to. Never its path, and never its contents.
    pub folder_name: String,
    pub files_left: i32,
    pub bytes_left: i64,
}

/// `GET /api/v1/public/upload-requests/{token}` — what this link is for.
///
/// Deliberately says nothing about what is already in the folder: the
/// point of the feature is sending without seeing.
pub async fn public_view(
    State(state): State<AppState>,
    Path(token): Path<String>,
) -> Result<Json<PublicRequestView>, ApiError> {
    let capability = resolve(&state, &token).await?;

    Ok(Json(PublicRequestView {
        title: capability.title,
        folder_name: capability.folder_name,
        files_left: capability.files_left.max(0),
        bytes_left: capability.bytes_left.max(0),
    }))
}

#[derive(Debug, Deserialize)]
pub struct SendQuery {
    /// What the sender calls the file. Used for its name only — never as
    /// a path — so a link can never write outside its own folder.
    pub name: String,
}

/// `POST /api/v1/public/upload-requests/{token}/files?name=`
///
/// Takes no session. The body is the file.
pub async fn send(
    State(state): State<AppState>,
    Path(token): Path<String>,
    Query(query): Query<SendQuery>,
    body: Body,
) -> Result<Json<ItemView>, ApiError> {
    let capability = resolve(&state, &token).await?;

    if capability.files_left <= 0 || capability.bytes_left <= 0 {
        return Err(ApiError::conflict(
            "This link has received everything it was set up to accept.",
        ));
    }

    // The name is treated as a name, never as a path: whatever arrives,
    // the file lands in this link's folder and nowhere else.
    let name = safe_name(&query.name)?;
    let folder = repository::item_in_library(state.db(), capability.library, capability.folder)
        .await
        .map_err(catalog_error)?;

    let requested = LibraryPath::parse(&format!("{}/{}", folder.path, name))
        .map_err(|error| ApiError::bad_request(error.to_string()))?;

    let storage = storage_for(&state, capability.library).await?;
    let destination = storage
        .available_path(&requested)
        .await
        .map_err(storage_error)?;

    let mut staged = storage
        .begin_upload(capability.bytes_left.max(0) as u64)
        .await
        .map_err(storage_error)?;

    let mut stream = body.into_data_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = match chunk {
            Ok(chunk) => chunk,
            Err(error) => {
                tracing::debug!(error = %error, "an upload through a link was cut short");
                staged.abort().await;
                return Err(ApiError::bad_request("The upload did not complete."));
            }
        };

        if let Err(error) = staged.write_chunk(&chunk).await {
            staged.abort().await;
            return Err(storage_error(error));
        }
    }

    let received = staged.written();

    storage
        .finish_upload(staged, &destination)
        .await
        .map_err(storage_error)?;

    // Counted after the bytes are on disk, and conditionally: two people
    // sending at once must not be able to push a link past its limits.
    let counted = sqlx::query(
        "UPDATE upload_requests
         SET received_files = received_files + 1,
             received_bytes = received_bytes + $2,
             last_used_at = now()
         WHERE id = $1
           AND received_files < max_files
           AND received_bytes + $2 <= max_bytes",
    )
    .bind(capability.id)
    .bind(received as i64)
    .execute(state.db())
    .await
    .map_err(|error| {
        tracing::warn!(error = %error, "could not record an upload through a link");
        ApiError::dependency_unavailable("database")
    })?
    .rows_affected();

    if counted == 0 {
        // The limit was reached by someone else while this was arriving.
        // The file is not kept: a link that says fifty files means fifty.
        let _ = storage.move_to_trash(&destination).await;

        return Err(ApiError::conflict(
            "This link has received everything it was set up to accept.",
        ));
    }

    let item = record_uploaded_file(&state, capability.library, &destination).await?;

    tracing::info!(bytes = received, "a file arrived through an upload link");

    Ok(Json(ItemView::from(&item)))
}

/// Reduces whatever a sender calls a file to a plain name.
///
/// Everything that could make it a path — separators, parent references,
/// nulls, control characters — is removed rather than rejected, because
/// a stranger sending a holiday photo should not have to debug a file
/// name. What cannot be salvaged is refused.
fn safe_name(raw: &str) -> Result<String, ApiError> {
    let candidate = raw.rsplit(['/', '\\']).next().unwrap_or_default();

    let cleaned: String = candidate
        .chars()
        .filter(|character| !character.is_control() && *character != '\0')
        .take(MAX_NAME)
        .collect();
    let cleaned = cleaned.trim().trim_matches('.').trim();

    if cleaned.is_empty() {
        return Err(ApiError::bad_request("That file needs a name."));
    }

    Ok(cleaned.to_owned())
}

/// Marks expired links revoked, so an owner's list stays truthful.
pub async fn purge_expired(pool: &PgPool) -> Result<u64, sqlx::Error> {
    let result = sqlx::query(
        "UPDATE upload_requests SET revoked_at = now()
         WHERE revoked_at IS NULL AND expires_at IS NOT NULL AND expires_at <= now()",
    )
    .execute(pool)
    .await?;

    Ok(result.rows_affected())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_name_is_a_name_and_never_a_path() {
        assert_eq!(safe_name("../../etc/passwd").unwrap(), "passwd");
        assert_eq!(safe_name("C:\\Users\\me\\photo.jpg").unwrap(), "photo.jpg");
        assert_eq!(safe_name("holiday.jpg").unwrap(), "holiday.jpg");
    }

    #[test]
    fn a_name_that_is_only_dots_or_blanks_is_refused() {
        assert!(safe_name("..").is_err());
        assert!(safe_name("   ").is_err());
        assert!(safe_name("").is_err());
        assert!(safe_name("/").is_err());
    }

    #[test]
    fn control_characters_are_stripped_rather_than_kept() {
        assert_eq!(safe_name("holi\u{7}day.jpg").unwrap(), "holiday.jpg");
    }
}
