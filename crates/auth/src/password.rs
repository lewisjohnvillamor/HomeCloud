//! Password credentials.
//!
//! Hashing is deliberately expensive, so every entry point here is
//! synchronous and CPU-bound by design: callers must move it off the
//! async executor (see [`crate::hash_password`]).

use argon2::password_hash::phc::PasswordHash;
use argon2::{Algorithm, Argon2, Params, PasswordHasher, PasswordVerifier, Version};
use rand::rngs::SysRng;
use rand::TryRng;

/// Shortest accepted password. Length beats composition rules; a
/// passphrase must be allowed and must not be truncated.
pub const MIN_PASSWORD_LENGTH: usize = 12;

/// Longest accepted password. Argon2 hashes the whole input, so an
/// unbounded value is a cheap way to burn server CPU.
pub const MAX_PASSWORD_LENGTH: usize = 1024;

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum PasswordError {
    #[error("password must be at least {MIN_PASSWORD_LENGTH} characters")]
    TooShort,
    #[error("password must be at most {MAX_PASSWORD_LENGTH} characters")]
    TooLong,
    #[error("password hashing failed")]
    Hashing,
}

/// Argon2id with parameters chosen for a home server: ~19 MiB of memory
/// and two passes, which is the low end of the OWASP guidance and still
/// far beyond what a GPU cracker handles cheaply.
fn hasher() -> Argon2<'static> {
    let params = Params::new(19 * 1024, 2, 1, None).expect("argon2 parameters are valid");
    Argon2::new(Algorithm::Argon2id, Version::V0x13, params)
}

/// Rejects passwords the policy will not accept. Separated from hashing
/// so the API can report a policy failure without doing the work.
pub fn check_policy(password: &str) -> Result<(), PasswordError> {
    // Characters, not bytes: a passphrase in any script gets the same
    // length budget.
    let length = password.chars().count();

    if length < MIN_PASSWORD_LENGTH {
        return Err(PasswordError::TooShort);
    }
    if length > MAX_PASSWORD_LENGTH {
        return Err(PasswordError::TooLong);
    }

    Ok(())
}

/// Hashes a password. Blocking and CPU-bound.
pub fn hash_password_blocking(password: &str) -> Result<String, PasswordError> {
    check_policy(password)?;

    // 16 random bytes: the salt only has to be unique per password, and
    // this is the size the PHC format expects for Argon2.
    let mut salt = [0u8; 16];
    SysRng
        .try_fill_bytes(&mut salt)
        .map_err(|_| PasswordError::Hashing)?;

    hasher()
        .hash_password_with_salt(password.as_bytes(), &salt)
        .map(|hash| hash.to_string())
        .map_err(|_| PasswordError::Hashing)
}

/// Verifies a password against a stored hash. Blocking and CPU-bound.
///
/// Returns `false` for every failure — wrong password, malformed hash,
/// unsupported algorithm — so a caller cannot accidentally turn a
/// storage problem into an authentication bypass.
pub fn verify_password_blocking(password: &str, stored_hash: &str) -> bool {
    let Ok(parsed) = PasswordHash::new(stored_hash) else {
        tracing::error!("stored password hash is unreadable");
        return false;
    };

    hasher()
        .verify_password(password.as_bytes(), &parsed)
        .is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_password_verifies_against_its_own_hash() {
        let hash = hash_password_blocking("correct horse battery staple").expect("hash");

        assert!(verify_password_blocking(
            "correct horse battery staple",
            &hash
        ));
    }

    #[test]
    fn a_wrong_password_is_rejected() {
        let hash = hash_password_blocking("correct horse battery staple").expect("hash");

        assert!(!verify_password_blocking(
            "correct horse battery stapler",
            &hash
        ));
    }

    #[test]
    fn hashes_are_salted_so_equal_passwords_differ() {
        let first = hash_password_blocking("correct horse battery staple").expect("hash");
        let second = hash_password_blocking("correct horse battery staple").expect("hash");

        assert_ne!(first, second);
    }

    #[test]
    fn short_passwords_are_refused() {
        assert_eq!(check_policy("short"), Err(PasswordError::TooShort));
        assert_eq!(
            hash_password_blocking("short"),
            Err(PasswordError::TooShort)
        );
    }

    #[test]
    fn absurdly_long_passwords_are_refused() {
        let long = "a".repeat(MAX_PASSWORD_LENGTH + 1);

        assert_eq!(check_policy(&long), Err(PasswordError::TooLong));
    }

    #[test]
    fn a_passphrase_in_any_script_gets_the_same_budget() {
        let passphrase = "правильная лошадь батарейка";

        assert!(check_policy(passphrase).is_ok());
    }

    #[test]
    fn a_corrupt_stored_hash_never_authenticates() {
        assert!(!verify_password_blocking(
            "correct horse battery staple",
            "not-a-hash"
        ));
        assert!(!verify_password_blocking("", ""));
    }
}
