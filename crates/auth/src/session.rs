//! Sessions.
//!
//! A session token is an opaque random string. Only its hash is stored,
//! so a database copy does not yield usable sessions, and lookups are by
//! hash rather than by a value an attacker could enumerate.

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use homecloud_domain::identity::UserId;
use rand::rngs::SysRng;
use rand::TryRng;
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use time::{Duration, OffsetDateTime};

/// How long a session stays valid without activity.
pub const SESSION_TTL: Duration = Duration::days(30);

/// How stale `last_seen_at` may get before it is written again. Without
/// this, every authenticated request would write to the database.
const LAST_SEEN_REFRESH_INTERVAL: Duration = Duration::hours(1);

/// 256 bits of entropy: not guessable, and short enough for a cookie.
const TOKEN_BYTES: usize = 32;

#[derive(Debug, thiserror::Error)]
pub enum SessionError {
    #[error("session token could not be generated")]
    Entropy,
    #[error(transparent)]
    Database(#[from] sqlx::Error),
}

/// A freshly minted token. The plain text exists only long enough to be
/// put in a cookie; it is never logged and never stored.
#[derive(Clone)]
pub struct SessionToken(String);

impl SessionToken {
    pub fn generate() -> Result<Self, SessionError> {
        let mut bytes = [0u8; TOKEN_BYTES];
        SysRng
            .try_fill_bytes(&mut bytes)
            .map_err(|_| SessionError::Entropy)?;

        Ok(Self(URL_SAFE_NO_PAD.encode(bytes)))
    }

    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Debug for SessionToken {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("SessionToken(<redacted>)")
    }
}

fn token_hash(token: &str) -> Vec<u8> {
    Sha256::digest(token.as_bytes()).to_vec()
}

/// An authenticated session as the server sees it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Session {
    pub user: UserId,
    pub expires_at: OffsetDateTime,
}

/// Issues a session for a user.
pub async fn create(pool: &PgPool, user: UserId) -> Result<SessionToken, SessionError> {
    let token = SessionToken::generate()?;
    let expires_at = OffsetDateTime::now_utc() + SESSION_TTL;

    sqlx::query("INSERT INTO sessions (user_id, token_hash, expires_at) VALUES ($1, $2, $3)")
        .bind(user.as_uuid())
        .bind(token_hash(token.expose()))
        .bind(expires_at)
        .execute(pool)
        .await?;

    Ok(token)
}

/// Resolves a token to a session, or `None` when it is unknown or
/// expired. Expiry is evaluated in the database so a wrong clock on an
/// application node cannot extend a session.
pub async fn authenticate(pool: &PgPool, token: &str) -> Result<Option<Session>, SessionError> {
    let row: Option<(uuid::Uuid, OffsetDateTime, OffsetDateTime)> = sqlx::query_as(
        "SELECT user_id, expires_at, last_seen_at
         FROM sessions
         WHERE token_hash = $1 AND expires_at > now()",
    )
    .bind(token_hash(token))
    .fetch_optional(pool)
    .await?;

    let Some((user, expires_at, last_seen_at)) = row else {
        return Ok(None);
    };

    if OffsetDateTime::now_utc() - last_seen_at > LAST_SEEN_REFRESH_INTERVAL {
        sqlx::query("UPDATE sessions SET last_seen_at = now() WHERE token_hash = $1")
            .bind(token_hash(token))
            .execute(pool)
            .await?;
    }

    Ok(Some(Session {
        user: UserId::from_uuid(user),
        expires_at,
    }))
}

/// Ends one session. Signing out must take effect immediately, so the
/// row is deleted rather than marked.
pub async fn revoke(pool: &PgPool, token: &str) -> Result<(), SessionError> {
    sqlx::query("DELETE FROM sessions WHERE token_hash = $1")
        .bind(token_hash(token))
        .execute(pool)
        .await?;

    Ok(())
}

/// Ends every session for a user, for "sign out everywhere" and for use
/// after a credential change.
pub async fn revoke_all_for_user(pool: &PgPool, user: UserId) -> Result<u64, SessionError> {
    let result = sqlx::query("DELETE FROM sessions WHERE user_id = $1")
        .bind(user.as_uuid())
        .execute(pool)
        .await?;

    Ok(result.rows_affected())
}

/// Removes expired rows. Called opportunistically; correctness does not
/// depend on it, because expiry is enforced on every lookup.
pub async fn purge_expired(pool: &PgPool) -> Result<u64, SessionError> {
    let result = sqlx::query("DELETE FROM sessions WHERE expires_at <= now()")
        .execute(pool)
        .await?;

    Ok(result.rows_affected())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokens_are_unique_and_long_enough_to_resist_guessing() {
        let first = SessionToken::generate().expect("token");
        let second = SessionToken::generate().expect("token");

        assert_ne!(first.expose(), second.expose());
        assert!(first.expose().len() >= 43, "{}", first.expose().len());
    }

    #[test]
    fn a_token_never_prints_itself() {
        let token = SessionToken::generate().expect("token");

        let rendered = format!("{token:?}");

        assert!(!rendered.contains(token.expose()));
    }

    #[test]
    fn hashing_is_stable_and_distinguishing() {
        assert_eq!(token_hash("abc"), token_hash("abc"));
        assert_ne!(token_hash("abc"), token_hash("abd"));
    }
}
