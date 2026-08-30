//! The catalog: what HomeCloud knows about the files in a library.
//!
//! Metadata here enriches the filesystem; it never becomes required to
//! recover a file. Everything in this crate can be rebuilt by rescanning
//! the library root.

// Application crates have no need for `unsafe`; an exception requires an ADR.
#![forbid(unsafe_code)]

pub mod item;
pub mod mutation;
pub mod repository;
pub mod scan;

pub use item::{Item, ItemKind};
pub use repository::{CatalogError, LibrarySummary};
pub use scan::{
    ScanSummary, DERIVATIVES_DIRECTORY, TRASH_DIRECTORY, UPLOAD_DIRECTORY, VERSIONS_DIRECTORY,
};
