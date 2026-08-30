//! Local-first AI providers.
//!
//! Everything here is optional at runtime. The product works with no
//! model configured — search, photos, memories and sharing are whole
//! without it — so nothing in this crate is a dependency of anything
//! else, only an addition to it. A provider that is not installed is an
//! ordinary answer, not an error.
//!
//! No model is named in domain logic. Callers ask for a capability and
//! get whichever provider the deployment has, or none.

pub mod ocr;
pub mod profile;

pub use profile::{Capabilities, Profile};

#[derive(Debug, thiserror::Error)]
pub enum AiError {
    /// The tool or model this provider needs is not on this machine.
    /// Expected on most deployments, and never logged as a failure.
    #[error("no provider is available for {0}")]
    Unavailable(&'static str),
    #[error("the file could not be read: {0}")]
    Unreadable(String),
    #[error("the provider failed: {0}")]
    Failed(String),
}
