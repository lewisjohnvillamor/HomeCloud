//! Background library scans.
//!
//! One scan per library at a time, tracked in process. A scan is
//! restartable and idempotent, so losing this state on restart costs
//! nothing but a rerun.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use homecloud_catalog::scan::{self, ScanSummary};
use homecloud_domain::identity::LibraryId;
use homecloud_storage::FilesystemStorage;
use serde::Serialize;
use sqlx::PgPool;
use time::OffsetDateTime;

#[derive(Debug, Clone, Serialize)]
pub struct ScanStatusView {
    pub running: bool,
    /// RFC 3339 timestamp of when the last scan finished.
    pub finished_at: Option<String>,
    pub last_summary: Option<ScanSummary>,
    /// A short, non-sensitive description of why the last scan failed.
    pub last_error: Option<String>,
}

#[derive(Debug, Default, Clone)]
struct ScanState {
    running: bool,
    finished_at: Option<OffsetDateTime>,
    last_summary: Option<ScanSummary>,
    last_error: Option<String>,
}

impl ScanState {
    fn view(&self) -> ScanStatusView {
        ScanStatusView {
            running: self.running,
            finished_at: self.finished_at.and_then(|value| {
                value
                    .format(&time::format_description::well_known::Rfc3339)
                    .ok()
            }),
            last_summary: self.last_summary,
            last_error: self.last_error.clone(),
        }
    }
}

/// Tracks scans across libraries.
#[derive(Debug, Default)]
pub struct ScanRegistry {
    states: Mutex<HashMap<uuid::Uuid, ScanState>>,
}

impl ScanRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn status(&self, library: LibraryId) -> ScanStatusView {
        self.lock()
            .get(&library.as_uuid())
            .cloned()
            .unwrap_or_default()
            .view()
    }

    /// Starts a scan unless one is already running for this library.
    /// Asking twice is not an error: the caller gets the running scan.
    pub fn start(
        self: &Arc<Self>,
        library: LibraryId,
        pool: PgPool,
        storage: FilesystemStorage,
    ) -> ScanStatusView {
        {
            let mut states = self.lock();
            let state = states.entry(library.as_uuid()).or_default();

            if state.running {
                return state.view();
            }

            state.running = true;
            state.last_error = None;
        }

        let registry = Arc::clone(self);
        tokio::spawn(async move {
            let outcome = scan::reconcile(&pool, library, &storage).await;

            let mut states = registry.lock();
            let state = states.entry(library.as_uuid()).or_default();
            state.running = false;
            state.finished_at = Some(OffsetDateTime::now_utc());

            match outcome {
                Ok(summary) => {
                    state.last_summary = Some(summary);
                    state.last_error = None;
                }
                Err(error) => {
                    tracing::error!(error = %error, "library scan failed");
                    // Deliberately generic: the detail is in the logs,
                    // not in a response body.
                    state.last_error = Some("The scan could not finish.".to_owned());
                }
            }
        });

        self.status(library)
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, HashMap<uuid::Uuid, ScanState>> {
        self.states.lock().unwrap_or_else(|poisoned| {
            tracing::warn!("scan registry lock was poisoned; continuing");
            poisoned.into_inner()
        })
    }
}
