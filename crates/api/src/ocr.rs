//! Reading text out of pictures, after a scan.
//!
//! Runs only when the library owner has turned private AI on, and only
//! on a machine that can actually do it. Bounded per pass like the text
//! extractor and the hasher beside it, so a library of forty thousand
//! photographs is read over many scans rather than in one burst that
//! makes the machine useless — the rule being that AI work must never
//! starve an upload or a preview.
//!
//! The text lands in the same `item_text` row a document extractor would
//! write, so search does not need to know where words came from. It is
//! marked as AI-derived so it can be removed on its own when someone
//! turns the feature off.

use homecloud_ai::ocr;
use homecloud_domain::identity::LibraryId;
use homecloud_storage::{FilesystemStorage, LibraryPath};
use sqlx::PgPool;

/// Images read in one pass. Deliberately small: recognition is seconds
/// per page, not milliseconds, and whatever is left is picked up by the
/// next scan.
const MAX_IMAGES_PER_PASS: i64 = 100;

#[derive(Debug, Default, PartialEq, Eq)]
pub struct OcrSummary {
    pub read: u64,
    pub empty: u64,
    pub failed: u64,
}

/// Reads text out of images that have not been read yet.
pub async fn read_library(
    pool: &PgPool,
    library: LibraryId,
    storage: &FilesystemStorage,
) -> OcrSummary {
    let mut summary = OcrSummary::default();

    let pending: Vec<(uuid::Uuid, String, i64, Option<time::OffsetDateTime>)> =
        match sqlx::query_as(
            "SELECT i.id, i.relative_path, i.size_bytes, i.modified_at
         FROM items i
         LEFT JOIN item_text t ON t.item_id = i.id
         WHERE i.library_id = $1
           AND i.kind = 'file'
           AND i.trashed_at IS NULL
           AND i.missing_since IS NULL
           AND i.content_type LIKE 'image/%'
           AND (
               t.item_id IS NULL
               OR (t.source <> 'ocr' AND t.status = 'unsupported')
               OR t.source_size IS DISTINCT FROM i.size_bytes
           )
         ORDER BY i.size_bytes
         LIMIT $2",
        )
        .bind(library.as_uuid())
        .bind(MAX_IMAGES_PER_PASS)
        .fetch_all(pool)
        .await
        {
            Ok(pending) => pending,
            Err(error) => {
                tracing::warn!(error = %error, "could not list images to read");
                return summary;
            }
        };

    for (id, path, size, modified) in pending {
        let Ok(parsed) = LibraryPath::parse(&path) else {
            summary.failed += 1;
            continue;
        };

        let Ok(resolved) = storage.resolve_existing(&parsed).await else {
            // Gone since the scan. Not an error worth recording.
            summary.failed += 1;
            continue;
        };

        match ocr::read_text(&resolved).await {
            Ok(text) => {
                // A picture of a beach has no text in it, and that is an
                // answer. It is recorded so the next scan does not open
                // the same photograph again forever.
                let empty = text.is_empty();
                let status = if empty { "unsupported" } else { "indexed" };

                if record(pool, library, id, &text, status, size, modified)
                    .await
                    .is_err()
                {
                    summary.failed += 1;
                } else if empty {
                    summary.empty += 1;
                } else {
                    summary.read += 1;
                }
            }
            Err(error) => {
                tracing::debug!(error = %error, "could not read text from an image");
                summary.failed += 1;
            }
        }
    }

    if summary.read > 0 || summary.failed > 0 {
        tracing::info!(
            read = summary.read,
            empty = summary.empty,
            failed = summary.failed,
            "text read from images"
        );
    }

    summary
}

/// Writes recognised text into the row search already reads.
async fn record(
    pool: &PgPool,
    library: LibraryId,
    item: uuid::Uuid,
    text: &str,
    status: &str,
    size: i64,
    modified: Option<time::OffsetDateTime>,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO item_text
            (item_id, library_id, content, status, source, source_size, source_modified_at)
         VALUES ($1, $2, $3, $4, 'ocr', $5, $6)
         ON CONFLICT (item_id) DO UPDATE
         SET content = $3, status = $4, source = 'ocr', source_size = $5,
             source_modified_at = $6, extracted_at = now()",
    )
    .bind(item)
    .bind(library.as_uuid())
    .bind(text)
    .bind(status)
    .bind(size)
    .bind(modified)
    .execute(pool)
    .await?;

    Ok(())
}
