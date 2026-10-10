//! Recognising what is in photographs, after a scan.
//!
//! Runs only when the library owner has chosen photo understanding, and
//! only when a model loaded at startup. Last in the scan pipeline, after
//! OCR, and bounded by time per pass, so a library of forty thousand
//! photographs is labelled over several scans rather than holding one
//! open for an afternoon. The model itself uses half the processor, so
//! an upload or a preview still gets the rest while it works.
//!
//! Labels go in `item_objects`, beside the text index rather than in it:
//! search reads both in one query, and the interface can still tell a
//! person "recognised in the photo" rather than present an inference as
//! something the file says.

use std::sync::Arc;
use std::time::{Duration, Instant};

use homecloud_ai::objects::{Label, ObjectDetector, CLASSES, INPUT_SIZE};
use homecloud_domain::identity::LibraryId;
use homecloud_media::{decode_for_analysis, MAX_SOURCE_BYTES};
use homecloud_storage::{FilesystemStorage, LibraryPath};
use sqlx::PgPool;

/// Images fetched from the database at a time.
const BATCH: i64 = 100;

/// Longest one scan spends labelling. Around five thousand photographs
/// on a six-core desktop processor; whatever is left waits for the next
/// scan, which is better than a scan that cannot finish.
const MAX_PASS_DURATION: Duration = Duration::from_secs(10 * 60);

#[derive(Debug, Default, PartialEq, Eq)]
pub struct LabelSummary {
    pub labelled: u64,
    pub unreadable: u64,
    pub failed: u64,
}

/// Labels images that have not been looked at by this model yet.
pub async fn label_library(
    pool: &PgPool,
    library: LibraryId,
    storage: &FilesystemStorage,
    detector: &Arc<ObjectDetector>,
) -> LabelSummary {
    let mut summary = LabelSummary::default();
    let started = Instant::now();
    // A file that fails for a reason worth retrying is not recorded, so
    // it would come straight back in the next batch. Skipping past what
    // this pass already tried keeps one broken file from being the
    // whole pass.
    let mut tried: Vec<uuid::Uuid> = Vec::new();

    'pass: while started.elapsed() < MAX_PASS_DURATION {
        let pending = match pending(pool, library, detector.model(), &tried).await {
            Ok(pending) if pending.is_empty() => break,
            Ok(pending) => pending,
            Err(error) => {
                tracing::warn!(error = %error, "could not list images to label");
                break;
            }
        };

        for image in pending {
            if started.elapsed() >= MAX_PASS_DURATION {
                break 'pass;
            }
            tried.push(image.id);

            match label_one(storage, detector, &image).await {
                Outcome::Labelled(labels) => {
                    match record(pool, library, detector.model(), &image, &labels).await {
                        Ok(()) => summary.labelled += 1,
                        Err(error) => {
                            tracing::warn!(error = %error, "could not save labels");
                            summary.failed += 1;
                        }
                    }
                }
                Outcome::Unreadable => {
                    // Damaged, too large, or a format the decoder does
                    // not read. Recorded, so the next scan does not open
                    // it again.
                    match record_unreadable(pool, library, detector.model(), &image).await {
                        Ok(()) => summary.unreadable += 1,
                        Err(_) => summary.failed += 1,
                    }
                }
                Outcome::Failed => summary.failed += 1,
            }
        }
    }

    if summary != LabelSummary::default() {
        tracing::info!(
            labelled = summary.labelled,
            unreadable = summary.unreadable,
            failed = summary.failed,
            seconds = started.elapsed().as_secs(),
            "photographs labelled"
        );
    }

    summary
}

struct PendingImage {
    id: uuid::Uuid,
    path: String,
    size: i64,
    modified: Option<time::OffsetDateTime>,
}

enum Outcome {
    Labelled(Vec<Label>),
    Unreadable,
    Failed,
}

async fn pending(
    pool: &PgPool,
    library: LibraryId,
    model: &str,
    tried: &[uuid::Uuid],
) -> Result<Vec<PendingImage>, sqlx::Error> {
    let rows: Vec<(uuid::Uuid, String, i64, Option<time::OffsetDateTime>)> = sqlx::query_as(
        "SELECT i.id, i.relative_path, i.size_bytes, i.modified_at
         FROM items i
         LEFT JOIN item_objects o ON o.item_id = i.id
         WHERE i.library_id = $1
           AND i.kind = 'file'
           AND i.trashed_at IS NULL
           AND i.missing_since IS NULL
           AND i.content_type LIKE 'image/%'
           AND (
               o.item_id IS NULL
               OR o.model <> $2
               OR o.source_size IS DISTINCT FROM i.size_bytes
               OR o.source_modified_at IS DISTINCT FROM i.modified_at
           )
           AND i.id <> ALL($3)
         ORDER BY i.modified_at DESC NULLS LAST, i.id
         LIMIT $4",
    )
    .bind(library.as_uuid())
    .bind(model)
    .bind(tried)
    .bind(BATCH)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|(id, path, size, modified)| PendingImage {
            id,
            path,
            size,
            modified,
        })
        .collect())
}

async fn label_one(
    storage: &FilesystemStorage,
    detector: &Arc<ObjectDetector>,
    image: &PendingImage,
) -> Outcome {
    // Refused from the catalog's size before a byte is read: the decoder
    // would refuse it anyway, after the whole file was in memory.
    if image.size < 0 || image.size as u64 > MAX_SOURCE_BYTES {
        return Outcome::Unreadable;
    }

    let Ok(parsed) = LibraryPath::parse(&image.path) else {
        return Outcome::Failed;
    };
    let Ok(resolved) = storage.resolve_existing(&parsed).await else {
        // Gone since the scan. The next scan will notice.
        return Outcome::Failed;
    };
    let Ok(bytes) = tokio::fs::read(&resolved).await else {
        return Outcome::Failed;
    };

    let detector = Arc::clone(detector);
    let work = tokio::task::spawn_blocking(move || {
        let pixels = decode_for_analysis(&bytes, INPUT_SIZE).map_err(|_| None)?;
        detector.recognise(&pixels).map_err(Some)
    });

    match work.await {
        Ok(Ok(labels)) => Outcome::Labelled(labels),
        Ok(Err(None)) => Outcome::Unreadable,
        Ok(Err(Some(error))) => {
            tracing::debug!(error = %error, "could not label a photograph");
            Outcome::Failed
        }
        Err(error) => {
            // A panic inside the decoder or the runtime. Logged and
            // counted; the next photograph still gets its turn.
            tracing::warn!(error = %error, "labelling a photograph panicked");
            Outcome::Failed
        }
    }
}

/// What one look at one photograph produced.
struct Finding<'a> {
    status: &'static str,
    labels: &'a [Label],
}

async fn record(
    pool: &PgPool,
    library: LibraryId,
    model: &str,
    image: &PendingImage,
    labels: &[Label],
) -> Result<(), sqlx::Error> {
    let finding = Finding {
        status: "labelled",
        labels,
    };
    write(pool, library, model, image, finding).await
}

async fn record_unreadable(
    pool: &PgPool,
    library: LibraryId,
    model: &str,
    image: &PendingImage,
) -> Result<(), sqlx::Error> {
    let finding = Finding {
        status: "unreadable",
        labels: &[],
    };
    write(pool, library, model, image, finding).await
}

async fn write(
    pool: &PgPool,
    library: LibraryId,
    model: &str,
    image: &PendingImage,
    finding: Finding<'_>,
) -> Result<(), sqlx::Error> {
    let names: Vec<&str> = finding.labels.iter().map(Label::name).collect();
    let confidences: Vec<f32> = finding
        .labels
        .iter()
        .map(|label| label.confidence)
        .collect();
    // Each label and the other names people use for it, from the same
    // vocabulary that produced the label.
    let words = finding
        .labels
        .iter()
        .map(|label| CLASSES[label.class].search_words())
        .collect::<Vec<_>>()
        .join(" ");

    sqlx::query(
        "INSERT INTO item_objects
            (item_id, library_id, model, status, labels, confidences, search_vector,
             source_size, source_modified_at)
         VALUES ($1, $2, $3, $4, $5, $6, to_tsvector('simple', $7), $8, $9)
         ON CONFLICT (item_id) DO UPDATE
         SET model = $3, status = $4, labels = $5, confidences = $6,
             search_vector = to_tsvector('simple', $7), source_size = $8,
             source_modified_at = $9, labelled_at = now()",
    )
    .bind(image.id)
    .bind(library.as_uuid())
    .bind(model)
    .bind(finding.status)
    .bind(&names)
    .bind(&confidences)
    .bind(&words)
    .bind(image.size)
    .bind(image.modified)
    .execute(pool)
    .await?;

    Ok(())
}
