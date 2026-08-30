//! Document text indexing.
//!
//! Runs after a library scan: reconciliation decides what exists, then
//! this reads the documents that are new or changed. Reading and parsing
//! are CPU- and I/O-bound, so both happen off the request executor, and
//! one pass is bounded so a huge library indexes over several scans
//! rather than in one unbounded burst.

use homecloud_domain::identity::LibraryId;
use homecloud_search::extract::{self, Status};
use homecloud_search::index::{self, IndexSummary};
use homecloud_storage::{FilesystemStorage, LibraryPath};
use sqlx::PgPool;

/// Documents read in one pass. A scan should finish in a reasonable time
/// even on a library full of PDFs; whatever is left is picked up by the
/// next one.
const MAX_DOCUMENTS_PER_PASS: i64 = 500;

/// Extracts text for documents whose text is missing or stale.
pub async fn index_library(
    pool: &PgPool,
    library: LibraryId,
    storage: &FilesystemStorage,
) -> IndexSummary {
    let mut summary = IndexSummary::default();

    let pending = match index::pending(pool, library, MAX_DOCUMENTS_PER_PASS).await {
        Ok(pending) => pending,
        Err(error) => {
            tracing::warn!(error = %error, "could not list documents to index");
            return summary;
        }
    };

    for document in pending {
        // Files this server cannot read are recorded once, so the next
        // scan does not open them again.
        if !extract::is_extractable(document.content_type.as_deref(), &document.name) {
            record(
                pool,
                library,
                &document,
                &unreadable(Status::Unsupported),
                &mut summary,
            )
            .await;
            continue;
        }

        let Ok(path) = LibraryPath::parse(&document.path) else {
            tracing::warn!("skipping a document whose stored path is not valid");
            continue;
        };

        let source = match storage.read_bounded(&path, extract::MAX_SOURCE_BYTES).await {
            Ok(source) => source,
            Err(homecloud_storage::StorageError::TooLarge) => {
                record(
                    pool,
                    library,
                    &document,
                    &unreadable(Status::TooLarge),
                    &mut summary,
                )
                .await;
                continue;
            }
            Err(error) => {
                tracing::debug!(error = %error, "a document could not be read for indexing");
                continue;
            }
        };

        let content_type = document.content_type.clone();
        let name = document.name.clone();
        let extraction = tokio::task::spawn_blocking(move || {
            extract::extract(&source, content_type.as_deref(), &name)
        })
        .await;

        match extraction {
            Ok(extraction) => record(pool, library, &document, &extraction, &mut summary).await,
            Err(error) => {
                tracing::warn!(error = %error, "the extraction task failed");
            }
        }
    }

    if summary != IndexSummary::default() {
        tracing::info!(
            indexed = summary.indexed,
            skipped = summary.skipped,
            failed = summary.failed,
            "document text indexed"
        );
    }

    summary
}

fn unreadable(status: Status) -> extract::Extraction {
    extract::Extraction {
        text: String::new(),
        status,
        truncated: false,
    }
}

async fn record(
    pool: &PgPool,
    library: LibraryId,
    document: &index::Pending,
    extraction: &extract::Extraction,
    summary: &mut IndexSummary,
) {
    match index::record(
        pool,
        library,
        document.item,
        extraction,
        document.size_bytes,
        document.modified_at,
    )
    .await
    {
        Ok(()) => summary.record(extraction.status),
        Err(error) => tracing::warn!(error = %error, "could not record extracted text"),
    }
}
