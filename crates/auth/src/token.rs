//! Bearer tokens.
//!
//! Sessions and share links are both bearer credentials: a random value
//! that grants access to whoever holds it. They therefore share one
//! implementation — 256 bits of entropy, stored only as a hash — so a
//! weakness cannot be fixed in one place and missed in the other.

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use rand::rngs::SysRng;
use rand::TryRng;
use sha2::{Digest, Sha256};

/// 256 bits: not guessable at internet scale, and short enough for a
/// cookie or a URL.
const TOKEN_BYTES: usize = 32;

/// Longest token accepted for lookup. Anything longer is not one of
/// ours, and hashing it would be wasted work.
pub const MAX_TOKEN_LENGTH: usize = 128;

#[derive(Debug, thiserror::Error)]
#[error("no entropy available to generate a token")]
pub struct EntropyError;

/// A freshly minted token. The plain text exists only long enough to be
/// handed to its holder; it is never logged and never stored.
#[derive(Clone)]
pub struct Token(String);

impl Token {
    pub fn generate() -> Result<Self, EntropyError> {
        let mut bytes = [0u8; TOKEN_BYTES];
        SysRng
            .try_fill_bytes(&mut bytes)
            .map_err(|_| EntropyError)?;

        Ok(Self(URL_SAFE_NO_PAD.encode(bytes)))
    }

    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Debug for Token {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Token(<redacted>)")
    }
}

/// Hash stored in place of a token.
///
/// SHA-256 rather than a password hash: the input already has 256 bits
/// of entropy, so there is nothing for an attacker to guess, and lookups
/// happen on every request.
pub fn hash(token: &str) -> Vec<u8> {
    Sha256::digest(token.as_bytes()).to_vec()
}

/// Whether a candidate is even shaped like one of our tokens.
pub fn is_plausible(token: &str) -> bool {
    !token.is_empty() && token.len() <= MAX_TOKEN_LENGTH
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokens_are_unique_and_long_enough_to_resist_guessing() {
        let first = Token::generate().expect("token");
        let second = Token::generate().expect("token");

        assert_ne!(first.expose(), second.expose());
        assert!(first.expose().len() >= 43, "{}", first.expose().len());
    }

    #[test]
    fn a_token_never_prints_itself() {
        let token = Token::generate().expect("token");

        assert!(!format!("{token:?}").contains(token.expose()));
    }

    #[test]
    fn hashing_is_stable_and_distinguishing() {
        assert_eq!(hash("abc"), hash("abc"));
        assert_ne!(hash("abc"), hash("abd"));
    }

    #[test]
    fn absurd_candidates_are_rejected_before_hashing() {
        assert!(!is_plausible(""));
        assert!(!is_plausible(&"a".repeat(MAX_TOKEN_LENGTH + 1)));
        assert!(is_plausible(Token::generate().expect("token").expose()));
    }
}
