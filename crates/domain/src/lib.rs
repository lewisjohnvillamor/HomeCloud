//! Core domain types for HomeCloud.
//!
//! This crate holds invariants that must hold regardless of transport,
//! database, or filesystem details. It intentionally has no I/O
//! dependencies so the rules can be tested in isolation.

// Application crates have no need for `unsafe`; an exception requires an ADR.
#![forbid(unsafe_code)]

pub mod identity;
pub mod library;
pub mod naming;

pub use identity::{ItemId, LibraryId, UserId};
pub use library::{Library, LibraryRole, Membership, MembershipError};
pub use naming::{LibraryName, NameError};
