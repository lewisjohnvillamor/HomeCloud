//! Thumbnail delivery.
//!
//! Derivatives are generated on first request and cached inside the
//! library root. Generation is CPU-bound, so it runs on a blocking pool;
//! a browser opening a grid of a hundred photos must not be able to stall
//! the request executor.

use axum::body::Body;
use axum::extract::{Path, Query, State};
use axum::http::{header, HeaderValue, StatusCode};
use axum::response::Response;
use homecloud_catalog::repository;
use homecloud_media::thumbnail::{is_thumbnailable, ThumbnailSize, DERIVATIVE_CONTENT_TYPE};
use homecloud_media::{generate_thumbnail, MediaError, MAX_SOURCE_BYTES};
use serde::Deserialize;

use crate::app::AppState;
use crate::auth::CurrentUser;
use crate::error::ApiError;
use crate::items::storage_error;
use crate::library::{catalog_error, parse_item, storage_for};

/// How long a browser may reuse a thumbnail.
///
/// Safe to cache for a long time because the cache key changes whenever
/// the source file does, and `private` keeps it out of shared caches:
/// these are someone's photos.
const CACHE_CONTROL: &str = "private, max-age=86400";

#[derive(Debug, Deserialize)]
pub struct ThumbnailQuery {
    #[serde(default)]
    pub size: Option<String>,
}

/// `GET /api/v1/items/{item}/thumbnail?size=small|medium|large`
pub async fn thumbnail(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Path(item): Path<String>,
    Query(query): Query<ThumbnailQuery>,
) -> Result<Response, ApiError> {
    let size = match query.size.as_deref() {
        None => ThumbnailSize::Small,
        Some(raw) => ThumbnailSize::parse(raw)
            .ok_or_else(|| ApiError::bad_request("Unknown thumbnail size."))?,
    };

    let item_id = parse_item(&item)?;
    let item = repository::item_for_user(state.db(), user, item_id)
        .await
        .map_err(catalog_error)?;

    if item.is_folder() || !is_thumbnailable(item.content_type.as_deref()) {
        return Err(ApiError::bad_request(
            "This item does not have a picture preview.",
        ));
    }
    if item.trashed_at.is_some() {
        return Err(ApiError::conflict(
            "This item is in the trash. Restore it to see it.",
        ));
    }

    let storage = storage_for(&state, item.library).await?;

    // The key changes when the file changes, so a replaced photo can
    // never be served from a stale derivative.
    let fingerprint = item
        .modified_at
        .map(|value| value.unix_timestamp())
        .unwrap_or_default();
    let key = format!(
        "{}-{}-{}-{}.jpg",
        item.id.as_uuid().simple(),
        size.as_str(),
        item.size_bytes,
        fingerprint
    );

    if let Some(cached) = storage.read_derivative(&key).await {
        return Ok(image_response(cached));
    }

    let source = storage
        .read_bounded(&item.path, MAX_SOURCE_BYTES)
        .await
        .map_err(storage_error)?;

    let generated = tokio::task::spawn_blocking(move || generate_thumbnail(&source, size))
        .await
        .map_err(|error| {
            tracing::error!(error = %error, "thumbnail task failed");
            ApiError::internal()
        })?
        .map_err(media_error)?;

    // A cache write failure costs a regeneration next time, so it is
    // logged rather than failing the request.
    if let Err(error) = storage.write_derivative(&key, &generated).await {
        tracing::warn!(error = %error, "thumbnail could not be cached");
    }

    Ok(image_response(generated))
}

fn image_response(bytes: Vec<u8>) -> Response {
    let length = bytes.len();
    let mut response = Response::new(Body::from(bytes));
    *response.status_mut() = StatusCode::OK;

    let headers = response.headers_mut();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static(DERIVATIVE_CONTENT_TYPE),
    );
    headers.insert(
        header::CONTENT_LENGTH,
        HeaderValue::from_str(&length.to_string()).expect("valid header"),
    );
    headers.insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static(CACHE_CONTROL),
    );
    headers.insert(
        header::CONTENT_DISPOSITION,
        HeaderValue::from_static("inline"),
    );

    response
}

/// A file that cannot be turned into a thumbnail is a fact about the
/// file, not a server failure: the client falls back to an icon.
fn media_error(error: MediaError) -> ApiError {
    match error {
        MediaError::TooLarge => ApiError::new(
            crate::error::ErrorCode::PayloadTooLarge,
            "This picture is too large to preview.",
        ),
        MediaError::UnsupportedFormat | MediaError::Damaged => {
            ApiError::bad_request("This picture could not be read.")
        }
        MediaError::Encoding => {
            tracing::error!("thumbnail encoding failed");
            ApiError::internal()
        }
    }
}
