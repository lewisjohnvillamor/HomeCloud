//! Backing up a phone's photographs.
//!
//! What this is not: a background service. A browser cannot read a
//! camera roll on its own — there is no web API that lets a page wake up
//! and notice you took a photograph — so an automatic backup needs an
//! application on the device, which this is not. Saying otherwise in the
//! interface would be the worst kind of lie, because somebody would
//! believe it and stop checking.
//!
//! What it is: the part that can be done honestly. Somebody opens a page
//! on their phone, selects everything, and only the photographs this
//! library does not already hold are sent. That is bearable to repeat
//! because the second run is nearly free — which is the whole job of
//! this module.
//!
//! "Already hold" is decided by name and size within the device's own
//! folder, deliberately including photographs that have been trashed. A
//! backup that resurrected the pictures you deleted last week would be
//! worse than one that missed some.

use axum::extract::{Path, State};
use axum::Json;
use homecloud_domain::identity::{LibraryId, UserId};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::app::AppState;
use crate::auth::CurrentUser;
use crate::error::ApiError;
use crate::library::{authorize, parse_library};

/// Where every device's folder lives, so a library gains one tidy
/// folder rather than one per phone scattered at the top level.
pub const BACKUP_ROOT: &str = "Phone backups";

/// Most files one check may ask about.
///
/// A camera roll is bigger than this; the client asks in batches. The
/// bound exists so a single request cannot be made enormous, and the
/// batching keeps the answer arriving steadily rather than after one
/// long silence.
const MAX_MANIFEST: usize = 2_000;

fn rfc3339(value: OffsetDateTime) -> String {
    value
        .format(&time::format_description::well_known::Rfc3339)
        .unwrap_or_default()
}

/// Longest device name accepted. Long enough for "Ada's work phone",
/// short enough that it cannot be used to build an unwieldy path.
const MAX_DEVICE_NAME: usize = 60;

#[derive(Debug, Serialize)]
pub struct DeviceView {
    pub id: String,
    pub name: String,
    /// Where its photographs land, shown plainly: these are ordinary
    /// files in an ordinary folder, and somebody should be able to go
    /// and look at them.
    pub folder: String,
    pub last_backup_at: Option<String>,
    /// How many photographs are in that folder now. Derived rather than
    /// counted into a column, because the filesystem is the truth and a
    /// stored tally would drift from it.
    pub photo_count: i64,
}

#[derive(Debug, Deserialize)]
pub struct DeviceRequest {
    pub name: String,
}

#[derive(Debug, Deserialize)]
pub struct CheckRequest {
    pub files: Vec<ManifestEntry>,
}

#[derive(Debug, Deserialize)]
pub struct ManifestEntry {
    pub name: String,
    pub size_bytes: i64,
}

#[derive(Debug, Serialize)]
pub struct CheckView {
    /// The names still to send, in the order they were offered.
    pub missing: Vec<String>,
    /// How many of the batch were already here. The number that makes a
    /// repeat backup feel instant rather than broken.
    pub already_here: usize,
    pub folder: String,
}

#[derive(Debug, Deserialize)]
pub struct FinishRequest {
    /// How many actually arrived, for the sentence shown afterwards.
    pub sent: i64,
}

/// `GET /api/v1/libraries/{library}/backup/devices`
pub async fn list(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Path(library): Path<String>,
) -> Result<Json<Vec<DeviceView>>, ApiError> {
    let library = parse_library(&library)?;
    authorize(&state, user, library).await?;

    let rows = sqlx::query_as::<_, DeviceRow>(
        "SELECT id, name, folder, last_backup_at
           FROM backup_devices
          WHERE library_id = $1 AND user_id = $2
          ORDER BY created_at",
    )
    .bind(library.as_uuid())
    .bind(user.as_uuid())
    .fetch_all(state.db())
    .await
    .map_err(|_| ApiError::internal())?;

    let mut devices = Vec::with_capacity(rows.len());
    for row in rows {
        let photo_count = count_in(state.db(), library, &row.folder).await?;
        devices.push(row.into_view(photo_count));
    }

    Ok(Json(devices))
}

/// `POST /api/v1/libraries/{library}/backup/devices`
///
/// Registering the same name twice returns the same device rather than
/// making a second one: somebody backing up from the same phone next
/// month should continue where they left off, and a phone whose name
/// they typed slightly differently is still that phone.
pub async fn register(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Path(library): Path<String>,
    Json(body): Json<DeviceRequest>,
) -> Result<Json<DeviceView>, ApiError> {
    let library = parse_library(&library)?;
    authorize(&state, user, library).await?;

    let name = device_name(&body.name)?;
    let folder = format!("{BACKUP_ROOT}/{name}");

    let row = sqlx::query_as::<_, DeviceRow>(
        "INSERT INTO backup_devices (library_id, user_id, name, folder)
         VALUES ($1, $2, $3, $4)
         ON CONFLICT (library_id, user_id, lower(name)) DO UPDATE
            SET name = EXCLUDED.name
         RETURNING id, name, folder, last_backup_at",
    )
    .bind(library.as_uuid())
    .bind(user.as_uuid())
    .bind(&name)
    .bind(&folder)
    .fetch_optional(state.db())
    .await
    .map_err(|_| ApiError::internal())?
    .ok_or_else(|| {
        // The folder index refused it, which means another member of
        // this library already backs a phone of this name up here.
        ApiError::conflict("Somebody else in this library already backs up a phone with that name.")
    })?;

    let photo_count = count_in(state.db(), library, &row.folder).await?;
    Ok(Json(row.into_view(photo_count)))
}

/// `POST /api/v1/libraries/{library}/backup/devices/{device}/check`
///
/// Answers "which of these do you not have?" for one batch.
pub async fn check(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Path((library, device)): Path<(String, String)>,
    Json(body): Json<CheckRequest>,
) -> Result<Json<CheckView>, ApiError> {
    let library = parse_library(&library)?;
    authorize(&state, user, library).await?;

    if body.files.len() > MAX_MANIFEST {
        return Err(ApiError::bad_request(format!(
            "Ask about at most {MAX_MANIFEST} files at a time."
        )));
    }

    let device = owned_device(state.db(), library, user, &device).await?;

    // One query for the whole batch, keyed on the unique path index
    // rather than a scan of the folder: a camera roll is large and this
    // runs every time somebody opens the page.
    let paths: Vec<String> = body
        .files
        .iter()
        .map(|file| format!("{}/{}", device.folder, file.name))
        .collect();

    let existing = sqlx::query_as::<_, (String, i64)>(
        // Trashed rows are deliberately included. A photograph somebody
        // deleted must not come back on the next backup, and the trashed
        // row is the only record that it was ever here.
        "SELECT relative_path, size_bytes
           FROM items
          WHERE library_id = $1 AND kind = 'file' AND relative_path = ANY($2)",
    )
    .bind(library.as_uuid())
    .bind(&paths)
    .fetch_all(state.db())
    .await
    .map_err(|_| ApiError::internal())?;

    let mut missing = Vec::new();
    for (file, path) in body.files.iter().zip(paths.iter()) {
        // Same name and same size is the same photograph. A same name
        // at a different size is a different one, and goes up under a
        // free name — the server never overwrites, so this needs no
        // special handling here.
        let here = existing
            .iter()
            .any(|(existing_path, size)| existing_path == path && *size == file.size_bytes);

        if !here {
            missing.push(file.name.clone());
        }
    }

    Ok(Json(CheckView {
        already_here: body.files.len() - missing.len(),
        missing,
        folder: device.folder,
    }))
}

/// `POST /api/v1/libraries/{library}/backup/devices/{device}/finish`
///
/// Records that a backup ran. Only the date is kept: what is actually in
/// the folder is read from the folder.
pub async fn finish(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Path((library, device)): Path<(String, String)>,
    Json(body): Json<FinishRequest>,
) -> Result<Json<DeviceView>, ApiError> {
    let library = parse_library(&library)?;
    authorize(&state, user, library).await?;

    if body.sent < 0 {
        return Err(ApiError::bad_request("A count cannot be negative."));
    }

    let device = owned_device(state.db(), library, user, &device).await?;

    let row = sqlx::query_as::<_, DeviceRow>(
        "UPDATE backup_devices SET last_backup_at = now()
          WHERE id = $1
         RETURNING id, name, folder, last_backup_at",
    )
    .bind(device.id)
    .fetch_one(state.db())
    .await
    .map_err(|_| ApiError::internal())?;

    let photo_count = count_in(state.db(), library, &row.folder).await?;
    Ok(Json(row.into_view(photo_count)))
}

/// `DELETE /api/v1/libraries/{library}/backup/devices/{device}`
///
/// Forgets the device. The photographs stay: somebody removing a phone
/// they no longer own is not asking to delete years of pictures, and if
/// they were, that is what the trash is for.
pub async fn forget(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Path((library, device)): Path<(String, String)>,
) -> Result<axum::http::StatusCode, ApiError> {
    let library = parse_library(&library)?;
    authorize(&state, user, library).await?;

    let device = owned_device(state.db(), library, user, &device).await?;

    sqlx::query("DELETE FROM backup_devices WHERE id = $1")
        .bind(device.id)
        .execute(state.db())
        .await
        .map_err(|_| ApiError::internal())?;

    Ok(axum::http::StatusCode::NO_CONTENT)
}

#[derive(sqlx::FromRow)]
struct DeviceRow {
    id: Uuid,
    name: String,
    folder: String,
    last_backup_at: Option<OffsetDateTime>,
}

impl DeviceRow {
    fn into_view(self, photo_count: i64) -> DeviceView {
        DeviceView {
            id: self.id.to_string(),
            name: self.name,
            folder: self.folder,
            last_backup_at: self.last_backup_at.map(rfc3339),
            photo_count,
        }
    }
}

/// Loads a device, refusing one belonging to somebody else.
///
/// Answered as "not found" rather than "not yours", so this cannot be
/// used to discover which phones other members back up here.
async fn owned_device(
    pool: &PgPool,
    library: LibraryId,
    user: UserId,
    device: &str,
) -> Result<DeviceRow, ApiError> {
    let id = Uuid::parse_str(device).map_err(|_| ApiError::not_found())?;

    sqlx::query_as::<_, DeviceRow>(
        "SELECT id, name, folder, last_backup_at
           FROM backup_devices
          WHERE id = $1 AND library_id = $2 AND user_id = $3",
    )
    .bind(id)
    .bind(library.as_uuid())
    .bind(user.as_uuid())
    .fetch_optional(pool)
    .await
    .map_err(|_| ApiError::internal())?
    .ok_or_else(ApiError::not_found)
}

/// How many files are in a device's folder now.
async fn count_in(pool: &PgPool, library: LibraryId, folder: &str) -> Result<i64, ApiError> {
    sqlx::query_scalar::<_, i64>(
        "SELECT count(*) FROM items
          WHERE library_id = $1 AND kind = 'file'
            AND trashed_at IS NULL AND missing_since IS NULL
            AND relative_path LIKE $2 || '/%'",
    )
    .bind(library.as_uuid())
    .bind(folder)
    .fetch_one(pool)
    .await
    .map_err(|_| ApiError::internal())
}

/// Checks a device name is something that can safely become a folder.
///
/// A device name is typed by a person and then used to build a path, so
/// it is checked here rather than trusted: a separator or a dot segment
/// would put somebody's photographs somewhere other than where the
/// interface says they went.
fn device_name(raw: &str) -> Result<String, ApiError> {
    let name = raw.trim();

    if name.is_empty() {
        return Err(ApiError::bad_request("A device needs a name."));
    }
    if name.chars().count() > MAX_DEVICE_NAME {
        return Err(ApiError::bad_request(format!(
            "A device name can be at most {MAX_DEVICE_NAME} characters."
        )));
    }
    if name.contains('/') || name.contains('\\') || name.contains('\0') {
        return Err(ApiError::bad_request(
            "A device name cannot contain a slash.",
        ));
    }
    if name == "." || name == ".." || name.starts_with('.') {
        return Err(ApiError::bad_request(
            "A device name cannot start with a dot.",
        ));
    }
    if name.chars().any(|c| c.is_control()) {
        return Err(ApiError::bad_request(
            "A device name cannot contain control characters.",
        ));
    }

    Ok(name.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_device_name_cannot_escape_its_folder() {
        // The name becomes a path segment, so everything that would make
        // it more than one segment is refused rather than rewritten:
        // silently mangling somebody's typing puts their photographs
        // somewhere they did not ask for.
        for hostile in [
            "../secrets",
            "a/b",
            "a\\b",
            "..",
            ".",
            ".hidden",
            "\u{0}",
            "line\nbreak",
            "   ",
            "",
        ] {
            assert!(
                device_name(hostile).is_err(),
                "accepted a hostile device name: {hostile:?}"
            );
        }
    }

    #[test]
    fn an_ordinary_device_name_is_kept_as_typed() {
        assert_eq!(device_name("  Ada's phone  ").unwrap(), "Ada's phone");
        assert_eq!(device_name("Pixel 8").unwrap(), "Pixel 8");
        // Not everybody names their phone in English.
        assert_eq!(device_name("아다의 폰").unwrap(), "아다의 폰");
    }

    #[test]
    fn a_device_name_is_bounded() {
        assert!(device_name(&"a".repeat(MAX_DEVICE_NAME)).is_ok());
        assert!(device_name(&"a".repeat(MAX_DEVICE_NAME + 1)).is_err());
    }
}
