//! Text extraction and search.
//!
//! Everything here is derived from the library and can be rebuilt by
//! rescanning it, which keeps the "metadata enriches, never gates" rule
//! true even for the search index.

// Application crates have no need for `unsafe`; an exception requires an ADR.
#![forbid(unsafe_code)]

pub mod extract;
pub mod index;
pub mod query;

pub use extract::{extract, is_extractable, Extraction, Status, MAX_SOURCE_BYTES};
pub use index::{IndexError, IndexSummary};
pub use query::{Hit, MatchKind};
