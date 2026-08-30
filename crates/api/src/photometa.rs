//! Reading what photos say about themselves.
//!
//! Runs after a library scan, beside document indexing: reconciliation
//! decides what exists, then this reads the header of each new or
//! changed picture to find when it was taken and what took it.
//!
//! Only the first few megabytes of each file are read, parsing happens
//! off the request executor, and one pass is bounded — a library of
//! forty thousand photos is enriched over several scans rather than in
//! one burst that blocks everything else.

use homecloud_domain::identity::{ItemId, LibraryId};
use homecloud_media::exif;
use homecloud_storage::{FilesystemStorage, LibraryPath};
use sqlx::PgPool;
use time::OffsetDateTime;

/// Photos read in one pass.
const MAX_PHOTOS_PER_PASS: i64 = 500;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct MetadataSummary {
    /// Files that turned out to have something to say.
    pub described: u64,
    /// Files read and found to say nothing. Recorded so the next scan
    /// does not open them again.
    pub silent: u64,
}

/// A photo whose header has not been read since it last changed.
struct Pending {
    item: ItemId,
    path: String,
}

/// Reads headers for photos whose metadata is missing or stale.
pub async fn describe_library(
    pool: &PgPool,
    library: LibraryId,
    storage: &FilesystemStorage,
) -> MetadataSummary {
    let mut summary = MetadataSummary::default();

    let pending = match pending(pool, library, MAX_PHOTOS_PER_PASS).await {
        Ok(pending) => pending,
        Err(error) => {
            tracing::warn!(error = %error, "could not list photos to describe");
            return summary;
        }
    };

    for photo in pending {
        let Ok(path) = LibraryPath::parse(&photo.path) else {
            tracing::warn!("skipping a photo whose stored path is not valid");
            continue;
        };

        // Only the header: metadata lives at the front of the file, and
        // a 60 MB raw file should not be read whole to find a date.
        let source = match storage
            .read_bounded_prefix(&path, exif::MAX_HEADER_BYTES as u64)
            .await
        {
            Ok(source) => source,
            Err(error) => {
                tracing::debug!(error = %error, "a photo could not be read for its metadata");
                continue;
            }
        };

        let Ok(metadata) = tokio::task::spawn_blocking(move || exif::read(&source)).await else {
            tracing::warn!("the metadata task failed");
            continue;
        };

        if metadata.is_empty() {
            summary.silent += 1;
        } else {
            summary.described += 1;
        }

        if let Err(error) = record(pool, photo.item, &metadata).await {
            tracing::warn!(error = %error, "could not store a photo's metadata");
        }
    }

    if summary != MetadataSummary::default() {
        tracing::info!(
            described = summary.described,
            silent = summary.silent,
            "photo metadata read"
        );
    }

    summary
}

/// Reads one photo's header immediately, for the upload path.
///
/// Returns the item with whatever was found already applied, so the
/// response to an upload carries the right date rather than one that
/// appears a scan later. A failure here is not worth refusing an upload
/// over: the next scan will pick the file up.
pub async fn describe_one(
    pool: &PgPool,
    storage: &FilesystemStorage,
    mut item: homecloud_catalog::Item,
) -> homecloud_catalog::Item {
    if !item.is_image() {
        return item;
    }

    let Ok(source) = storage
        .read_bounded_prefix(&item.path, exif::MAX_HEADER_BYTES as u64)
        .await
    else {
        return item;
    };

    let Ok(metadata) = tokio::task::spawn_blocking(move || exif::read(&source)).await else {
        return item;
    };

    if let Err(error) = record(pool, item.id, &metadata).await {
        tracing::warn!(error = %error, "could not store an uploaded photo's metadata");
        return item;
    }

    item.taken_at = metadata.taken_at;
    item.camera = metadata.camera;
    item.latitude = metadata.latitude;
    item.longitude = metadata.longitude;

    item
}

/// Photos that have never been read, or that changed since they were.
async fn pending(
    pool: &PgPool,
    library: LibraryId,
    limit: i64,
) -> Result<Vec<Pending>, sqlx::Error> {
    let rows: Vec<(uuid::Uuid, String)> = sqlx::query_as(
        "SELECT id, relative_path FROM items
         WHERE library_id = $1
           AND kind = 'file'
           AND content_type LIKE 'image/%'
           AND trashed_at IS NULL
           AND missing_since IS NULL
           AND (photo_metadata_at IS NULL OR photo_metadata_at < indexed_at)
         ORDER BY indexed_at DESC
         LIMIT $2",
    )
    .bind(library.as_uuid())
    .bind(limit)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|(id, path)| Pending {
            item: ItemId::from_uuid(id),
            path,
        })
        .collect())
}

/// Stores what was found, including nothing.
///
/// `photo_metadata_at` is set either way: a photo with no header is a
/// finished question, not one to ask again on the next scan.
async fn record(
    pool: &PgPool,
    item: ItemId,
    metadata: &exif::PhotoMetadata,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE items
         SET taken_at = $2, camera = $3, photo_metadata_at = $4,
             latitude = $5, longitude = $6
         WHERE id = $1",
    )
    .bind(item.as_uuid())
    .bind(metadata.taken_at)
    .bind(metadata.camera.as_deref())
    .bind(OffsetDateTime::now_utc())
    .bind(metadata.latitude)
    .bind(metadata.longitude)
    .execute(pool)
    .await?;

    Ok(())
}
