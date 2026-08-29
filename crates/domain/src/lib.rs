//! Core domain types for HomeCloud.
//!
//! This crate holds invariants that must hold regardless of transport,
//! database, or filesystem details. It intentionally has no I/O
//! dependencies so the rules can be tested in isolation.

pub mod naming;

pub use naming::{LibraryName, NameError};
