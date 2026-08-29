//! HomeCloud HTTP API.
//!
//! The crate is organised so the router can be constructed and tested
//! without binding a socket: `main` only wires configuration, logging,
//! and the listener.

// Application crates have no need for `unsafe`; an exception requires an ADR.
#![forbid(unsafe_code)]

pub mod app;
pub mod auth;
pub mod bootstrap;
pub mod config;
pub mod db;
pub mod error;
pub mod health;
pub mod items;
pub mod library;
pub mod observability;
pub mod ratelimit;
pub mod scanjob;
pub mod security;
pub mod transfers;
pub mod view;

/// Name reported by the API in logs and startup output.
pub const SERVICE_NAME: &str = "homecloud-api";
