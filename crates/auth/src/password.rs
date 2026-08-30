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

    hash_secret(password)
}

/// Hashes any secret with the shared Argon2 parameters.
fn hash_secret(secret: &str) -> Result<String, PasswordError> {
    // 16 random bytes: the salt only has to be unique per password, and
    // this is the size the PHC format expects for Argon2.
    let mut salt = [0u8; 16];
    SysRng
        .try_fill_bytes(&mut salt)
        .map_err(|_| PasswordError::Hashing)?;

    hasher()
        .hash_password_with_salt(secret.as_bytes(), &salt)
        .map(|hash| hash.to_string())
        .map_err(|_| PasswordError::Hashing)
}

/// Verifies a password against a stored hash. Blocking and CPU-bound.
///
/// Returns `false` for every failure — wrong password, malformed hash,
/// unsupported algorithm — so a caller cannot accidentally turn a
/// storage problem into an authentication bypass.
pub fn verify_password_blocking(password: &str, stored_hash: &str) -> bool {
    verify_secret(password, stored_hash)
}

fn verify_secret(secret: &str, stored_hash: &str) -> bool {
    let Ok(parsed) = PasswordHash::new(stored_hash) else {
        tracing::error!("a stored secret hash is unreadable");
        return false;
    };

    hasher().verify_password(secret.as_bytes(), &parsed).is_ok()
}

/// A recovery code: five groups of five characters from an alphabet
/// with no look-alikes, so it can be written down and read back.
///
/// About 116 bits of entropy — far beyond guessing — while staying
/// something a person can copy onto paper and keep in a drawer, which is
/// the actual recovery story for a server in someone's house.
pub const RECOVERY_ALPHABET: &[u8] = b"ABCDEFGHJKMNPQRSTUVWXYZ23456789";
const RECOVERY_GROUPS: usize = 5;
const RECOVERY_GROUP_LEN: usize = 5;

/// Generates a recovery code in `XXXXX-XXXXX-…` form.
pub fn generate_recovery_code() -> Result<String, PasswordError> {
    generate_code(RECOVERY_GROUPS, RECOVERY_GROUP_LEN)
}

/// Generates a grouped code from the look-alike-free alphabet.
///
/// Rejection sampling rather than `byte % len`: 31 does not divide 256,
/// so the remainder would make the first few characters of the alphabet
/// slightly likelier than the rest. It costs nothing to be exact.
pub fn generate_code(groups: usize, group_len: usize) -> Result<String, PasswordError> {
    use rand::rngs::SysRng;
    use rand::TryRng;

    let alphabet_len = RECOVERY_ALPHABET.len();
    // Largest multiple of the alphabet that fits in a byte; anything at
    // or above it is drawn again.
    let ceiling = (256 / alphabet_len) * alphabet_len;

    let mut characters = Vec::with_capacity(groups * group_len);
    let mut buffer = [0u8; 64];

    while characters.len() < groups * group_len {
        SysRng
            .try_fill_bytes(&mut buffer)
            .map_err(|_| PasswordError::Hashing)?;

        for byte in buffer {
            if characters.len() == groups * group_len {
                break;
            }
            if usize::from(byte) < ceiling {
                characters.push(RECOVERY_ALPHABET[usize::from(byte) % alphabet_len] as char);
            }
        }
    }

    Ok(characters
        .chunks(group_len)
        .map(|group| group.iter().collect::<String>())
        .collect::<Vec<_>>()
        .join("-"))
}

/// Normalises a code a person typed: case and separators are noise.
pub fn normalise_recovery_code(code: &str) -> String {
    code.chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .map(|character| character.to_ascii_uppercase())
        .collect()
}

/// Hashes a recovery code. Blocking and CPU-bound, like a password.
pub fn hash_recovery_code_blocking(code: &str) -> Result<String, PasswordError> {
    hash_secret(&normalise_recovery_code(code))
}

/// Verifies a recovery code against its stored hash.
pub fn verify_recovery_code_blocking(code: &str, stored_hash: &str) -> bool {
    verify_secret(&normalise_recovery_code(code), stored_hash)
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

#[cfg(test)]
mod recovery_tests {
    use super::*;

    #[test]
    fn a_recovery_code_is_readable_and_verifies() {
        let code = generate_recovery_code().expect("code");

        // Five groups of five, so it can be written on paper.
        assert_eq!(code.split('-').count(), 5);
        assert!(code.split('-').all(|group| group.len() == 5), "{code}");

        let hash = hash_recovery_code_blocking(&code).expect("hash");
        assert!(verify_recovery_code_blocking(&code, &hash));
    }

    #[test]
    fn codes_are_unique() {
        let first = generate_recovery_code().expect("code");
        let second = generate_recovery_code().expect("code");

        assert_ne!(first, second);
    }

    #[test]
    fn typing_it_back_forgives_case_and_separators() {
        let code = generate_recovery_code().expect("code");
        let hash = hash_recovery_code_blocking(&code).expect("hash");

        let as_typed = code.to_lowercase().replace('-', " ");

        assert!(verify_recovery_code_blocking(&as_typed, &hash));
    }

    #[test]
    fn a_wrong_code_is_refused() {
        let hash =
            hash_recovery_code_blocking(&generate_recovery_code().expect("code")).expect("hash");

        assert!(!verify_recovery_code_blocking(
            "ABCDE-FGHJK-MNPQR-STUVW-XYZ23",
            &hash
        ));
        assert!(!verify_recovery_code_blocking("", &hash));
    }

    #[test]
    fn the_alphabet_has_no_look_alike_characters() {
        let alphabet = String::from_utf8(RECOVERY_ALPHABET.to_vec()).expect("ascii");

        for confusing in ['O', '0', 'I', '1', 'L'] {
            assert!(
                !alphabet.contains(confusing),
                "`{confusing}` is easy to misread on paper"
            );
        }
    }
}
