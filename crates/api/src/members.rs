//! Library membership: invitations, members, and who may administer.
//!
//! Membership is the authorization boundary of the whole system, so the
//! rules live in one place: `require_owner` is the only thing that
//! decides whether a caller may change who has access.

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::Json;
use homecloud_auth::token::{self, Token};
use homecloud_domain::identity::{LibraryId, UserId};
use homecloud_domain::library::LibraryRole;
use serde::{Deserialize, Serialize};
use time::{Duration, OffsetDateTime};

use crate::app::AppState;
use crate::auth::{self, CurrentUser};
use crate::error::ApiError;
use crate::library::{authorize, parse_library};

/// Longest invitation lifetime. An invitation is a way into someone's
/// files; one that never expires is a liability sitting in an inbox.
const MAX_EXPIRY_DAYS: i64 = 30;
const DEFAULT_EXPIRY_DAYS: i64 = 7;

fn rfc3339(value: OffsetDateTime) -> String {
    value
        .format(&time::format_description::well_known::Rfc3339)
        .unwrap_or_default()
}

/// The caller's role in a library, or `None` when they are not a member.
async fn role_of(
    state: &AppState,
    user: UserId,
    library: LibraryId,
) -> Result<Option<LibraryRole>, ApiError> {
    let role: Option<String> = sqlx::query_scalar(
        "SELECT role FROM library_members WHERE library_id = $1 AND user_id = $2",
    )
    .bind(library.as_uuid())
    .bind(user.as_uuid())
    .fetch_optional(state.db())
    .await
    .map_err(|error| {
        tracing::warn!(error = %error, "membership lookup failed");
        ApiError::dependency_unavailable("database")
    })?;

    Ok(role.and_then(|role| LibraryRole::parse(&role).ok()))
}

/// The single place that decides whether a caller may change who has
/// access to a library.
///
/// A non-member gets "not found" — whether a library exists is itself
/// private — while a member who is not the owner is told plainly that
/// this is the owner's to do.
async fn require_owner(state: &AppState, user: UserId, library: LibraryId) -> Result<(), ApiError> {
    match role_of(state, user, library).await? {
        Some(LibraryRole::Owner) => Ok(()),
        Some(LibraryRole::Member) => Err(ApiError::forbidden(
            "Only the library owner can manage who has access.",
        )),
        None => Err(ApiError::not_found()),
    }
}

#[derive(Debug, Serialize)]
pub struct MemberView {
    pub user_id: String,
    pub display_name: String,
    pub role: &'static str,
    pub added_at: String,
    /// True for the caller, so the UI can avoid offering "remove me".
    pub is_you: bool,
}

/// `GET /api/v1/libraries/{library}/members`
///
/// Any member may see who else is in the library: people sharing a
/// library should not have to guess who can read their files.
pub async fn list_members(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Path(library): Path<String>,
) -> Result<Json<Vec<MemberView>>, ApiError> {
    let library = parse_library(&library)?;
    authorize(&state, user, library).await?;

    let rows: Vec<(uuid::Uuid, String, String, OffsetDateTime)> = sqlx::query_as(
        "SELECT u.id, u.display_name, m.role, m.added_at
         FROM library_members m
         JOIN users u ON u.id = m.user_id
         WHERE m.library_id = $1
         ORDER BY (m.role = 'owner') DESC, lower(u.display_name)",
    )
    .bind(library.as_uuid())
    .fetch_all(state.db())
    .await
    .map_err(|error| {
        tracing::warn!(error = %error, "member listing failed");
        ApiError::dependency_unavailable("database")
    })?;

    Ok(Json(
        rows.into_iter()
            .map(|(id, display_name, role, added_at)| MemberView {
                user_id: id.to_string(),
                display_name,
                role: match LibraryRole::parse(&role) {
                    Ok(LibraryRole::Owner) => "owner",
                    _ => "member",
                },
                added_at: rfc3339(added_at),
                is_you: id == user.as_uuid(),
            })
            .collect(),
    ))
}

/// `DELETE /api/v1/libraries/{library}/members/{user}`
pub async fn remove_member(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Path((library, member)): Path<(String, String)>,
) -> Result<StatusCode, ApiError> {
    let library = parse_library(&library)?;
    require_owner(&state, user, library).await?;

    let member = uuid::Uuid::parse_str(&member).map_err(|_| ApiError::not_found())?;

    // The owner cannot be removed — the domain says so, and the query
    // says so too, so neither layer alone can be talked out of it.
    let removed = sqlx::query(
        "DELETE FROM library_members
         WHERE library_id = $1 AND user_id = $2 AND role <> 'owner'",
    )
    .bind(library.as_uuid())
    .bind(member)
    .execute(state.db())
    .await
    .map_err(|error| {
        tracing::warn!(error = %error, "member removal failed");
        ApiError::dependency_unavailable("database")
    })?
    .rows_affected();

    if removed == 0 {
        return Err(ApiError::not_found());
    }

    // Access ends now, not when their session happens to expire.
    if let Err(error) =
        homecloud_auth::session::revoke_all_for_user(state.db(), UserId::from_uuid(member)).await
    {
        tracing::warn!(error = %error, "could not end the removed member's sessions");
    }

    tracing::info!("a member was removed from a library");

    Ok(StatusCode::NO_CONTENT)
}

#[derive(Debug, Deserialize)]
pub struct CreateInvitationRequest {
    pub expires_in_days: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct InvitationView {
    pub id: String,
    pub library_name: String,
    pub invited_by: String,
    pub created_at: String,
    pub expires_at: String,
    /// Only on the response that creates it; the token is not stored.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token: Option<String>,
}

/// `POST /api/v1/libraries/{library}/invitations`
pub async fn create_invitation(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Path(library): Path<String>,
    Json(request): Json<CreateInvitationRequest>,
) -> Result<Json<InvitationView>, ApiError> {
    let library = parse_library(&library)?;
    require_owner(&state, user, library).await?;

    let days = request.expires_in_days.unwrap_or(DEFAULT_EXPIRY_DAYS);
    if !(1..=MAX_EXPIRY_DAYS).contains(&days) {
        return Err(ApiError::bad_request(format!(
            "Choose an expiry between 1 and {MAX_EXPIRY_DAYS} days."
        )));
    }

    let token = Token::generate().map_err(|_| {
        tracing::error!("no entropy available for an invitation token");
        ApiError::internal()
    })?;
    let expires_at = OffsetDateTime::now_utc() + Duration::days(days);

    let row: (uuid::Uuid, OffsetDateTime, String, String) = sqlx::query_as(
        "WITH inserted AS (
             INSERT INTO invitations (library_id, created_by, token_hash, role, expires_at)
             VALUES ($1, $2, $3, 'member', $4)
             RETURNING id, created_at, library_id, created_by
         )
         SELECT inserted.id, inserted.created_at, l.name, u.display_name
         FROM inserted
         JOIN libraries l ON l.id = inserted.library_id
         JOIN users u ON u.id = inserted.created_by",
    )
    .bind(library.as_uuid())
    .bind(user.as_uuid())
    .bind(token::hash(token.expose()))
    .bind(expires_at)
    .fetch_one(state.db())
    .await
    .map_err(|error| {
        tracing::error!(error = %error, "invitation creation failed");
        ApiError::internal()
    })?;

    tracing::info!("an invitation was created");

    Ok(Json(InvitationView {
        id: row.0.to_string(),
        library_name: row.2,
        invited_by: row.3,
        created_at: rfc3339(row.1),
        expires_at: rfc3339(expires_at),
        token: Some(token.expose().to_owned()),
    }))
}

/// `GET /api/v1/libraries/{library}/invitations` — pending invitations.
pub async fn list_invitations(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Path(library): Path<String>,
) -> Result<Json<Vec<InvitationView>>, ApiError> {
    let library = parse_library(&library)?;
    require_owner(&state, user, library).await?;

    let rows: Vec<(uuid::Uuid, OffsetDateTime, OffsetDateTime, String, String)> = sqlx::query_as(
        "SELECT i.id, i.created_at, i.expires_at, l.name, u.display_name
         FROM invitations i
         JOIN libraries l ON l.id = i.library_id
         JOIN users u ON u.id = i.created_by
         WHERE i.library_id = $1
           AND i.accepted_at IS NULL
           AND i.revoked_at IS NULL
           AND i.expires_at > now()
         ORDER BY i.created_at DESC",
    )
    .bind(library.as_uuid())
    .fetch_all(state.db())
    .await
    .map_err(|error| {
        tracing::warn!(error = %error, "invitation listing failed");
        ApiError::dependency_unavailable("database")
    })?;

    Ok(Json(
        rows.into_iter()
            .map(
                |(id, created_at, expires_at, library_name, invited_by)| InvitationView {
                    id: id.to_string(),
                    library_name,
                    invited_by,
                    created_at: rfc3339(created_at),
                    expires_at: rfc3339(expires_at),
                    token: None,
                },
            )
            .collect(),
    ))
}

/// `DELETE /api/v1/invitations/{invitation}`
pub async fn revoke_invitation(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Path(invitation): Path<String>,
) -> Result<StatusCode, ApiError> {
    let invitation = uuid::Uuid::parse_str(&invitation).map_err(|_| ApiError::not_found())?;

    let revoked = sqlx::query(
        "UPDATE invitations i
         SET revoked_at = now()
         WHERE i.id = $1
           AND i.revoked_at IS NULL
           AND EXISTS (
               SELECT 1 FROM library_members m
               WHERE m.library_id = i.library_id
                 AND m.user_id = $2
                 AND m.role = 'owner'
           )",
    )
    .bind(invitation)
    .bind(user.as_uuid())
    .execute(state.db())
    .await
    .map_err(|error| {
        tracing::warn!(error = %error, "invitation revocation failed");
        ApiError::dependency_unavailable("database")
    })?
    .rows_affected();

    if revoked == 0 {
        return Err(ApiError::not_found());
    }

    Ok(StatusCode::NO_CONTENT)
}

// --- Accepting an invitation. No session required. ---

#[derive(Debug, Serialize)]
pub struct InvitationPreview {
    pub library_name: String,
    pub invited_by: String,
    pub expires_at: String,
}

/// `GET /api/v1/invitations/{token}`
///
/// Tells someone holding an invitation what it is for, and nothing else:
/// a library name and who sent it. No file names, no member list, no id.
pub async fn preview_invitation(
    State(state): State<AppState>,
    Path(token): Path<String>,
) -> Result<Json<InvitationPreview>, ApiError> {
    let (_, library_name, invited_by, expires_at) = load_open_invitation(&state, &token).await?;

    Ok(Json(InvitationPreview {
        library_name,
        invited_by,
        expires_at: rfc3339(expires_at),
    }))
}

#[derive(Debug, Deserialize)]
pub struct AcceptInvitationRequest {
    /// Supplied when the person does not have an account yet.
    pub display_name: Option<String>,
    pub password: Option<String>,
}

/// `POST /api/v1/invitations/{token}/accept`
///
/// Creates an account when one is needed, adds the membership, and signs
/// the person in. The whole thing is one transaction so a half-accepted
/// invitation cannot exist.
pub async fn accept_invitation(
    State(state): State<AppState>,
    parts: axum::http::request::Parts,
    Path(token): Path<String>,
    Json(request): Json<AcceptInvitationRequest>,
) -> Result<axum::response::Response, ApiError> {
    let (invitation, _, _, _) = load_open_invitation(&state, &token).await?;

    // Someone already signed in accepts as themselves; anyone else has
    // to create the account the invitation is for.
    let existing = auth::current_user(&state, &parts).await;

    let (user, display_name) = match existing {
        Some(user) => {
            let display_name: String =
                sqlx::query_scalar("SELECT display_name FROM users WHERE id = $1")
                    .bind(user.as_uuid())
                    .fetch_one(state.db())
                    .await
                    .map_err(|error| {
                        tracing::warn!(error = %error, "user lookup failed");
                        ApiError::dependency_unavailable("database")
                    })?;

            (user, display_name)
        }
        None => {
            let display_name = request
                .display_name
                .as_deref()
                .map(str::trim)
                .filter(|name| !name.is_empty() && name.chars().count() <= 64)
                .ok_or_else(|| ApiError::bad_request("Enter a name of 1 to 64 characters."))?
                .to_owned();
            let password = request
                .password
                .clone()
                .ok_or_else(|| ApiError::bad_request("Choose a password."))?;

            homecloud_auth::password::check_policy(&password)
                .map_err(|error| ApiError::bad_request(error.to_string()))?;
            let password_hash = homecloud_auth::hash_password(password)
                .await
                .map_err(|_| ApiError::internal())?;

            let created: uuid::Uuid = sqlx::query_scalar(
                "INSERT INTO users (display_name, password_hash) VALUES ($1, $2) RETURNING id",
            )
            .bind(&display_name)
            .bind(&password_hash)
            .fetch_one(state.db())
            .await
            .map_err(|error| {
                if auth::is_unique_violation(&error) {
                    ApiError::conflict("An account with that name already exists.")
                } else {
                    tracing::error!(error = %error, "account creation failed");
                    ApiError::internal()
                }
            })?;

            (UserId::from_uuid(created), display_name)
        }
    };

    // Claim the invitation and add the membership together: claiming it
    // first means a second acceptance finds nothing to claim.
    let mut tx = state.db().begin().await.map_err(|error| {
        tracing::error!(error = %error, "could not begin the acceptance transaction");
        ApiError::internal()
    })?;

    let claimed: Option<uuid::Uuid> = sqlx::query_scalar(
        "UPDATE invitations
         SET accepted_at = now(), accepted_by = $2
         WHERE id = $1 AND accepted_at IS NULL AND revoked_at IS NULL AND expires_at > now()
         RETURNING library_id",
    )
    .bind(invitation)
    .bind(user.as_uuid())
    .fetch_optional(&mut *tx)
    .await
    .map_err(|error| {
        tracing::error!(error = %error, "invitation claim failed");
        ApiError::internal()
    })?;

    let Some(library) = claimed else {
        return Err(ApiError::not_found());
    };

    sqlx::query(
        "INSERT INTO library_members (library_id, user_id, role)
         VALUES ($1, $2, 'member')
         ON CONFLICT (library_id, user_id) DO NOTHING",
    )
    .bind(library)
    .bind(user.as_uuid())
    .execute(&mut *tx)
    .await
    .map_err(|error| {
        tracing::error!(error = %error, "membership creation failed");
        ApiError::internal()
    })?;

    tx.commit().await.map_err(|error| {
        tracing::error!(error = %error, "could not commit the acceptance");
        ApiError::internal()
    })?;

    tracing::info!("an invitation was accepted");

    auth::issue_session(&state, user, display_name).await
}

/// Loads an invitation that is still open, or reports "not found".
///
/// Unknown, expired, revoked, and already-accepted all look the same: a
/// visitor must not learn that an invitation once existed.
async fn load_open_invitation(
    state: &AppState,
    token: &str,
) -> Result<(uuid::Uuid, String, String, OffsetDateTime), ApiError> {
    if !token::is_plausible(token) {
        return Err(ApiError::not_found());
    }

    let row: Option<(uuid::Uuid, String, String, OffsetDateTime)> = sqlx::query_as(
        "SELECT i.id, l.name, u.display_name, i.expires_at
         FROM invitations i
         JOIN libraries l ON l.id = i.library_id
         JOIN users u ON u.id = i.created_by
         WHERE i.token_hash = $1
           AND i.accepted_at IS NULL
           AND i.revoked_at IS NULL
           AND i.expires_at > now()",
    )
    .bind(token::hash(token))
    .fetch_optional(state.db())
    .await
    .map_err(|error| {
        tracing::warn!(error = %error, "invitation lookup failed");
        ApiError::dependency_unavailable("database")
    })?;

    row.ok_or_else(ApiError::not_found)
}
