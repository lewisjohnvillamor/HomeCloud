//! Credentials, sessions, and access decisions.

// Application crates have no need for `unsafe`; an exception requires an ADR.
#![forbid(unsafe_code)]

pub mod password;
pub mod session;
pub mod token;

pub use password::{PasswordError, MAX_PASSWORD_LENGTH, MIN_PASSWORD_LENGTH};
pub use session::{Session, SessionError, SessionToken, SESSION_TTL};
pub use token::Token;

/// Hashes a password without blocking the async executor.
pub async fn hash_password(password: String) -> Result<String, PasswordError> {
    tokio::task::spawn_blocking(move || password::hash_password_blocking(&password))
        .await
        .unwrap_or(Err(PasswordError::Hashing))
}

/// Verifies a password without blocking the async executor.
pub async fn verify_password(password: String, stored_hash: String) -> bool {
    tokio::task::spawn_blocking(move || password::verify_password_blocking(&password, &stored_hash))
        .await
        .unwrap_or(false)
}
