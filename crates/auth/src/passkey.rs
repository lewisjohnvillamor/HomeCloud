//! Passkeys.
//!
//! A passkey is a second kind of credential against the same session
//! model as a password: registering one does not change how
//! authorization works, and an account may have either or both.

use std::sync::Arc;

use homecloud_domain::identity::UserId;
use sqlx::PgPool;
use time::OffsetDateTime;
use url::Url;
use uuid::Uuid;
use webauthn_rs::prelude::{
    CredentialID, Passkey, PasskeyAuthentication, PasskeyRegistration, PublicKeyCredential,
    RegisterPublicKeyCredential,
};
use webauthn_rs::{Webauthn, WebauthnBuilder};

#[derive(Debug, thiserror::Error)]
pub enum PasskeyError {
    #[error("passkeys are not configured on this server")]
    NotConfigured,
    #[error("the browser's response could not be verified")]
    Rejected,
    #[error("this passkey is already registered")]
    AlreadyRegistered,
    #[error("no passkey is registered for that account")]
    NoCredentials,
    #[error(transparent)]
    Database(#[from] sqlx::Error),
}

/// The relying party: which origin passkeys are bound to.
///
/// WebAuthn ties a credential to a domain, so this cannot be guessed
/// from a request — a server that trusted the `Host` header here would
/// let an attacker mint credentials for their own domain.
#[derive(Clone)]
pub struct PasskeyService {
    webauthn: Arc<Webauthn>,
}

impl std::fmt::Debug for PasskeyService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("PasskeyService")
    }
}

impl PasskeyService {
    /// Builds the service for a public origin, such as
    /// `https://home.example` or `http://localhost:3000`.
    pub fn new(public_origin: &str) -> Result<Self, PasskeyError> {
        let origin = Url::parse(public_origin).map_err(|_| PasskeyError::NotConfigured)?;
        let rp_id = origin
            .host_str()
            .ok_or(PasskeyError::NotConfigured)?
            .to_owned();

        let webauthn = WebauthnBuilder::new(&rp_id, &origin)
            .map_err(|_| PasskeyError::NotConfigured)?
            .rp_name("HomeCloud")
            .build()
            .map_err(|_| PasskeyError::NotConfigured)?;

        Ok(Self {
            webauthn: Arc::new(webauthn),
        })
    }

    /// Starts registration for a signed-in user.
    ///
    /// Existing credentials are excluded so an authenticator cannot be
    /// registered twice and silently shadow itself.
    pub async fn start_registration(
        &self,
        pool: &PgPool,
        user: UserId,
        display_name: &str,
    ) -> Result<(serde_json::Value, PasskeyRegistration), PasskeyError> {
        let existing: Vec<CredentialID> = credentials_for(pool, user)
            .await?
            .into_iter()
            .map(|(_, passkey)| passkey.cred_id().clone())
            .collect();

        let (challenge, state) = self
            .webauthn
            .start_passkey_registration(user.as_uuid(), display_name, display_name, Some(existing))
            .map_err(|error| {
                tracing::warn!(error = %error, "could not start passkey registration");
                PasskeyError::Rejected
            })?;

        let challenge = serde_json::to_value(challenge).map_err(|_| PasskeyError::Rejected)?;

        Ok((challenge, state))
    }

    /// Completes registration and stores the credential.
    pub async fn finish_registration(
        &self,
        pool: &PgPool,
        user: UserId,
        nickname: &str,
        response: RegisterPublicKeyCredential,
        state: &PasskeyRegistration,
    ) -> Result<Uuid, PasskeyError> {
        let passkey = self
            .webauthn
            .finish_passkey_registration(&response, state)
            .map_err(|error| {
                tracing::warn!(error = %error, "passkey registration was rejected");
                PasskeyError::Rejected
            })?;

        let encoded = serde_json::to_value(&passkey).map_err(|_| PasskeyError::Rejected)?;

        let stored: Result<(Uuid,), sqlx::Error> = sqlx::query_as(
            "INSERT INTO credentials (user_id, credential_id, passkey, nickname)
             VALUES ($1, $2, $3, $4)
             RETURNING id",
        )
        .bind(user.as_uuid())
        .bind(passkey.cred_id().as_ref())
        .bind(&encoded)
        .bind(nickname)
        .fetch_one(pool)
        .await;

        match stored {
            Ok((id,)) => Ok(id),
            Err(error) if is_unique_violation(&error) => Err(PasskeyError::AlreadyRegistered),
            Err(error) => Err(PasskeyError::Database(error)),
        }
    }

    /// Starts authentication for an account that has passkeys.
    pub async fn start_authentication(
        &self,
        pool: &PgPool,
        user: UserId,
    ) -> Result<(serde_json::Value, PasskeyAuthentication), PasskeyError> {
        let passkeys: Vec<Passkey> = credentials_for(pool, user)
            .await?
            .into_iter()
            .map(|(_, passkey)| passkey)
            .collect();

        if passkeys.is_empty() {
            return Err(PasskeyError::NoCredentials);
        }

        let (challenge, state) = self
            .webauthn
            .start_passkey_authentication(&passkeys)
            .map_err(|error| {
                tracing::warn!(error = %error, "could not start passkey authentication");
                PasskeyError::Rejected
            })?;

        let challenge = serde_json::to_value(challenge).map_err(|_| PasskeyError::Rejected)?;

        Ok((challenge, state))
    }

    /// Completes authentication, updating the credential's counter.
    ///
    /// The counter is how an authenticator proves it has not been cloned;
    /// storing it back is what makes that check meaningful next time.
    pub async fn finish_authentication(
        &self,
        pool: &PgPool,
        user: UserId,
        response: PublicKeyCredential,
        state: &PasskeyAuthentication,
    ) -> Result<(), PasskeyError> {
        let result = self
            .webauthn
            .finish_passkey_authentication(&response, state)
            .map_err(|error| {
                tracing::warn!(error = %error, "passkey authentication was rejected");
                PasskeyError::Rejected
            })?;

        for (id, mut passkey) in credentials_for(pool, user).await? {
            if passkey.update_credential(&result).is_some() {
                let encoded = serde_json::to_value(&passkey).map_err(|_| PasskeyError::Rejected)?;

                sqlx::query(
                    "UPDATE credentials SET passkey = $2, last_used_at = now() WHERE id = $1",
                )
                .bind(id)
                .bind(&encoded)
                .execute(pool)
                .await?;
                break;
            }
        }

        Ok(())
    }
}

/// A registered passkey, as its owner sees it.
#[derive(Debug, Clone)]
pub struct RegisteredPasskey {
    pub id: Uuid,
    pub nickname: String,
    pub created_at: OffsetDateTime,
    pub last_used_at: Option<OffsetDateTime>,
}

pub async fn list_for_user(
    pool: &PgPool,
    user: UserId,
) -> Result<Vec<RegisteredPasskey>, PasskeyError> {
    let rows: Vec<(Uuid, String, OffsetDateTime, Option<OffsetDateTime>)> = sqlx::query_as(
        "SELECT id, nickname, created_at, last_used_at
         FROM credentials
         WHERE user_id = $1
         ORDER BY created_at",
    )
    .bind(user.as_uuid())
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(
            |(id, nickname, created_at, last_used_at)| RegisteredPasskey {
                id,
                nickname,
                created_at,
                last_used_at,
            },
        )
        .collect())
}

/// Removes a passkey. Scoped by user so one account cannot delete
/// another's credential.
pub async fn remove(pool: &PgPool, user: UserId, credential: Uuid) -> Result<bool, PasskeyError> {
    let removed = sqlx::query("DELETE FROM credentials WHERE id = $1 AND user_id = $2")
        .bind(credential)
        .bind(user.as_uuid())
        .execute(pool)
        .await?
        .rows_affected();

    Ok(removed > 0)
}

/// Whether an account has any passkey, which decides whether the sign-in
/// screen offers one.
pub async fn user_has_passkeys(pool: &PgPool, user: UserId) -> Result<bool, PasskeyError> {
    let exists: bool =
        sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM credentials WHERE user_id = $1)")
            .bind(user.as_uuid())
            .fetch_one(pool)
            .await?;

    Ok(exists)
}

async fn credentials_for(
    pool: &PgPool,
    user: UserId,
) -> Result<Vec<(Uuid, Passkey)>, PasskeyError> {
    let rows: Vec<(Uuid, serde_json::Value)> =
        sqlx::query_as("SELECT id, passkey FROM credentials WHERE user_id = $1")
            .bind(user.as_uuid())
            .fetch_all(pool)
            .await?;

    Ok(rows
        .into_iter()
        .filter_map(|(id, encoded)| match serde_json::from_value(encoded) {
            Ok(passkey) => Some((id, passkey)),
            Err(error) => {
                // A credential this build cannot read is skipped rather
                // than failing every sign-in for that account.
                tracing::error!(error = %error, "a stored passkey could not be read");
                None
            }
        })
        .collect())
}

fn is_unique_violation(error: &sqlx::Error) -> bool {
    matches!(
        error.as_database_error().and_then(|error| error.code()),
        Some(code) if code == "23505"
    )
}
