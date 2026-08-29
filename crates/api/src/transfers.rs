//! File transfer: streaming downloads with range support, and streaming
//! uploads that only become visible once complete.
//!
//! Neither direction buffers a whole file in memory: a personal cloud
//! holds videos, and a 4 GiB file must cost about as much memory as a
//! 4 KiB one.

use axum::body::Body;
use axum::extract::{Path, Query, State};
use axum::http::{header, HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use futures::StreamExt;
use homecloud_storage::MutableStorage;
use percent_encoding::{utf8_percent_encode, NON_ALPHANUMERIC};
use serde::Deserialize;
use tokio::io::{AsyncReadExt, AsyncSeekExt};
use tokio_util::io::ReaderStream;

use crate::app::AppState;
use crate::auth::CurrentUser;
use crate::error::ApiError;
use crate::items::{parse_path, record_uploaded_file, storage_error};
use crate::library::{authorize, catalog_error, parse_item, parse_library, storage_for};
use crate::view::ItemView;

/// Largest single upload accepted. Bounded so one request cannot fill
/// the disk; resumable uploads for larger files are their own feature.
pub const MAX_UPLOAD_BYTES: u64 = 5 * 1024 * 1024 * 1024;

/// Content types served for inline display.
///
/// Everything else is downloaded as an opaque attachment. SVG is
/// deliberately absent: it can carry script, and the library is full of
/// files other people sent the user.
const INLINE_TYPES: &[&str] = &[
    "image/jpeg",
    "image/png",
    "image/gif",
    "image/webp",
    "image/avif",
    "image/bmp",
    "video/mp4",
    "video/webm",
    "audio/mpeg",
    "audio/ogg",
    "audio/wav",
    "text/plain",
    "application/pdf",
];

#[derive(Debug, Deserialize)]
pub struct UploadQuery {
    /// Destination path, including the file name.
    pub path: String,
}

/// `POST /api/v1/libraries/{library}/upload?path=...`
///
/// The body is the file. Bytes are streamed to a staging file inside the
/// root and renamed into place at the end, so an interrupted upload
/// never leaves a partial file in the library.
pub async fn upload(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Path(library): Path<String>,
    Query(query): Query<UploadQuery>,
    body: Body,
) -> Result<Json<ItemView>, ApiError> {
    let library = parse_library(&library)?;
    authorize(&state, user, library).await?;

    let requested = parse_path(&query.path)?;
    let storage = storage_for(&state, library).await?;

    // Never overwrite: a name collision produces "report (2).pdf", the
    // same as a desktop file manager.
    let destination = storage
        .available_path(&requested)
        .await
        .map_err(storage_error)?;

    let mut staged = storage
        .begin_upload(MAX_UPLOAD_BYTES)
        .await
        .map_err(storage_error)?;

    let mut stream = body.into_data_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = match chunk {
            Ok(chunk) => chunk,
            Err(error) => {
                tracing::warn!(error = %error, "upload stream failed");
                staged.abort().await;
                return Err(ApiError::bad_request("The upload did not complete."));
            }
        };

        if let Err(error) = staged.write_chunk(&chunk).await {
            staged.abort().await;
            return Err(storage_error(error));
        }
    }

    storage
        .finish_upload(staged, &destination)
        .await
        .map_err(storage_error)?;

    let item = record_uploaded_file(&state, library, &destination).await?;

    Ok(Json(ItemView::from(&item)))
}

/// `GET /api/v1/items/{item}/content`
///
/// Supports a single byte range, which is what browsers use to seek in
/// audio and video.
pub async fn download(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Path(item): Path<String>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    let item_id = parse_item(&item)?;
    let item = homecloud_catalog::repository::item_for_user(state.db(), user, item_id)
        .await
        .map_err(catalog_error)?;

    if item.is_folder() {
        return Err(ApiError::bad_request(
            "A folder has no content to download.",
        ));
    }

    let storage = storage_for(&state, item.library).await?;

    // A trashed item's bytes live under the trash directory; only the
    // item that was trashed directly records where they went.
    let location = if item.trashed_at.is_some() {
        homecloud_catalog::mutation::trash_location(state.db(), item.library, item.id)
            .await
            .map_err(catalog_error)?
            .ok_or_else(|| {
                ApiError::conflict("This item is in the trash. Restore it to open it.")
            })?
    } else {
        item.path.clone()
    };

    let (mut file, size) = storage.open_file(&location).await.map_err(storage_error)?;

    let range = headers
        .get(header::RANGE)
        .and_then(|value| value.to_str().ok())
        .map(|value| parse_range(value, size));

    let (status, start, length) = match range {
        None => (StatusCode::OK, 0, size),
        Some(Some((start, end))) => (StatusCode::PARTIAL_CONTENT, start, end - start + 1),
        Some(None) => {
            // Unsatisfiable range: the specification asks for 416 with the
            // real size, so the client can retry correctly.
            let mut response = ApiError::new(
                crate::error::ErrorCode::BadRequest,
                "That byte range is not available.",
            )
            .to_response(None);
            *response.status_mut() = StatusCode::RANGE_NOT_SATISFIABLE;
            response.headers_mut().insert(
                header::CONTENT_RANGE,
                HeaderValue::from_str(&format!("bytes */{size}")).expect("valid header"),
            );
            return Ok(response);
        }
    };

    if start > 0 {
        file.seek(std::io::SeekFrom::Start(start))
            .await
            .map_err(|error| {
                tracing::warn!(error = %error, "seek failed");
                ApiError::internal()
            })?;
    }

    let stream = ReaderStream::new(file.take(length));
    let mut response = Response::new(Body::from_stream(stream));
    *response.status_mut() = status;

    let headers = response.headers_mut();
    headers.insert(
        header::CONTENT_TYPE,
        content_type_header(item.content_type.as_deref()),
    );
    headers.insert(
        header::CONTENT_DISPOSITION,
        disposition_header(&item.name, item.content_type.as_deref()),
    );
    headers.insert(
        header::CONTENT_LENGTH,
        HeaderValue::from_str(&length.to_string()).expect("valid header"),
    );
    headers.insert(header::ACCEPT_RANGES, HeaderValue::from_static("bytes"));
    if status == StatusCode::PARTIAL_CONTENT {
        headers.insert(
            header::CONTENT_RANGE,
            HeaderValue::from_str(&format!("bytes {start}-{}/{size}", start + length - 1))
                .expect("valid header"),
        );
    }

    Ok(response.into_response())
}

/// Parses a single `bytes=` range.
///
/// Returns `None` for a range that cannot be satisfied, and treats
/// anything it does not understand — multiple ranges, other units — as
/// "send the whole file", which is what the specification allows.
fn parse_range(header: &str, size: u64) -> Option<(u64, u64)> {
    if size == 0 {
        return None;
    }

    let spec = header.strip_prefix("bytes=")?.trim();
    if spec.contains(',') {
        return None;
    }

    let (start, end) = spec.split_once('-')?;

    let (start, end) = match (start.trim(), end.trim()) {
        // `bytes=-500`: the last 500 bytes.
        ("", suffix) => {
            let length: u64 = suffix.parse().ok()?;
            if length == 0 {
                return None;
            }
            (size.saturating_sub(length), size - 1)
        }
        // `bytes=500-`: from 500 to the end.
        (start, "") => (start.parse().ok()?, size - 1),
        (start, end) => (start.parse().ok()?, end.parse::<u64>().ok()?.min(size - 1)),
    };

    (start <= end && start < size).then_some((start, end))
}

/// The type a download is served as.
///
/// Anything outside the inline allowlist becomes an opaque byte stream,
/// so a file that happens to be named `.html` cannot be rendered as a
/// page on the library's own origin.
fn content_type_header(content_type: Option<&str>) -> HeaderValue {
    let value = content_type
        .filter(|value| INLINE_TYPES.contains(value))
        .unwrap_or("application/octet-stream");

    HeaderValue::from_str(value).unwrap_or(HeaderValue::from_static("application/octet-stream"))
}

/// Builds a `Content-Disposition` that survives a hostile file name.
///
/// The name is percent-encoded into the `filename*` form, so quotes,
/// newlines, or semicolons in a name cannot inject header syntax.
fn disposition_header(name: &str, content_type: Option<&str>) -> HeaderValue {
    let inline = content_type.is_some_and(|value| INLINE_TYPES.contains(&value));
    let kind = if inline { "inline" } else { "attachment" };
    let encoded = utf8_percent_encode(name, NON_ALPHANUMERIC);

    HeaderValue::from_str(&format!("{kind}; filename*=UTF-8''{encoded}"))
        .unwrap_or(HeaderValue::from_static("attachment"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_full_range_is_parsed() {
        assert_eq!(parse_range("bytes=0-99", 1000), Some((0, 99)));
    }

    #[test]
    fn an_open_ended_range_runs_to_the_end() {
        assert_eq!(parse_range("bytes=900-", 1000), Some((900, 999)));
    }

    #[test]
    fn a_suffix_range_counts_back_from_the_end() {
        assert_eq!(parse_range("bytes=-100", 1000), Some((900, 999)));
    }

    #[test]
    fn a_range_past_the_end_is_clamped() {
        assert_eq!(parse_range("bytes=0-99999", 1000), Some((0, 999)));
    }

    #[test]
    fn an_unsatisfiable_range_is_rejected() {
        assert_eq!(parse_range("bytes=1000-1001", 1000), None);
        assert_eq!(parse_range("bytes=5-1", 1000), None);
        assert_eq!(parse_range("bytes=0-0", 0), None);
    }

    #[test]
    fn unsupported_range_forms_fall_back_to_the_whole_file() {
        assert_eq!(parse_range("items=0-10", 1000), None);
        assert_eq!(parse_range("bytes=0-10,20-30", 1000), None);
        assert_eq!(parse_range("nonsense", 1000), None);
    }

    #[test]
    fn a_hostile_file_name_cannot_inject_header_syntax() {
        let header = disposition_header("evil\"; drop=1\r\nX-Injected: yes.txt", None);
        let rendered = header.to_str().expect("ascii");

        assert!(!rendered.contains('\r'));
        assert!(!rendered.contains('\n'));
        assert!(rendered.starts_with("attachment; filename*=UTF-8''"));
    }

    #[test]
    fn only_allowlisted_types_are_served_inline() {
        assert_eq!(content_type_header(Some("image/png")), "image/png");
        assert_eq!(
            content_type_header(Some("image/svg+xml")),
            "application/octet-stream"
        );
        assert_eq!(
            content_type_header(Some("text/html")),
            "application/octet-stream"
        );
        assert_eq!(content_type_header(None), "application/octet-stream");
    }
}
