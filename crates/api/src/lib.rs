//! HomeCloud HTTP API.
//!
//! The crate is organised so the router can be constructed and tested
//! without binding a socket: `main` only wires configuration, logging,
//! and the listener.

pub mod app;
pub mod config;
pub mod db;
pub mod error;
pub mod health;
pub mod observability;

/// Name reported by the API in logs and startup output.
pub const SERVICE_NAME: &str = "homecloud-api";
