//! Public share links.
//!
//! A share is a capability, not a session. It carries read access to one
//! item — and, for a folder, the things inside it — and nothing else.
//! Everything a session can do that a share cannot is enforced by the
//! routes themselves: the public handlers below never take a
//! `CurrentUser`, and never reach a query that is not scoped to the
//! shared item's subtree.

use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::Response;
use axum::Json;
use homecloud_auth::token::{self, Token};
use homecloud_catalog::repository::{self};
use homecloud_catalog::Item;
use homecloud_domain::identity::{ItemId, LibraryId, UserId};
use homecloud_media::thumbnail::{is_thumbnailable, ThumbnailSize};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use time::{Duration, OffsetDateTime};

use crate::app::AppState;
use crate::auth::CurrentUser;
use crate::error::ApiError;
use crate::library::{authorize, catalog_error, parse_item, storage_for};
use crate::view::ItemView;

/// Longest expiry a client may ask for. A link that outlives the memory
/// of having made it is how libraries leak.
const MAX_EXPIRY_DAYS: i64 = 365;

/// Shortest password a link may carry.
const MIN_SHARE_PASSWORD_LENGTH: usize = 8;

/// How long an unlocked link stays unlocked. Long enough to browse a
/// shared folder and download from it, short enough that a URL left in a
/// browser history is not a lasting key.
const UNLOCK_TTL: std::time::Duration = std::time::Duration::from_secs(60 * 60);

/// Most unlocks held at once, so a flood of attempts cannot grow this
/// map without limit.
const MAX_UNLOCKS: usize = 1024;

/// Unlocked share links, by the opaque key handed to the visitor.
///
/// Held in memory: an unlock is worth an hour, and losing them on
/// restart costs a re-entry of the password.
#[derive(Debug, Default)]
pub struct ShareUnlocks {
    entries: std::sync::Mutex<std::collections::HashMap<String, (uuid::Uuid, std::time::Instant)>>,
}

impl ShareUnlocks {
    pub fn new() -> Self {
        Self::default()
    }

    fn issue(&self, share: uuid::Uuid) -> Result<String, ApiError> {
        let key = Token::generate()
            .map_err(|_| {
                tracing::error!("no entropy available for a share unlock");
                ApiError::internal()
            })?
            .expose()
            .to_owned();

        let mut entries = self.lock();
        entries.retain(|_, (_, issued)| issued.elapsed() < UNLOCK_TTL);
        if entries.len() >= MAX_UNLOCKS {
            entries.clear();
        }
        entries.insert(key.clone(), (share, std::time::Instant::now()));

        Ok(key)
    }

    /// Whether this key unlocks that share, right now.
    fn permits(&self, key: &str, share: uuid::Uuid) -> bool {
        self.lock()
            .get(key)
            .is_some_and(|(unlocked, issued)| *unlocked == share && issued.elapsed() < UNLOCK_TTL)
    }

    fn lock(
        &self,
    ) -> std::sync::MutexGuard<
        '_,
        std::collections::HashMap<String, (uuid::Uuid, std::time::Instant)>,
    > {
        self.entries.lock().unwrap_or_else(|poisoned| {
            tracing::warn!("the share unlock lock was poisoned; continuing");
            poisoned.into_inner()
        })
    }
}

/// Generates a share token. Returned once, at creation; only its hash is
/// stored afterwards, exactly as for a session.
fn generate_token() -> Result<Token, ApiError> {
    Token::generate().map_err(|_| {
        tracing::error!("no entropy available for a share token");
        ApiError::internal()
    })
}

fn token_hash(token: &str) -> Vec<u8> {
    token::hash(token)
}

#[derive(Debug, Deserialize)]
pub struct CreateShareRequest {
    /// How long the link should work for. Absent means until revoked.
    pub expires_in_days: Option<i64>,
    /// Optional second factor, for a link sent over a channel the sender
    /// does not fully trust.
    pub password: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ShareView {
    pub id: String,
    pub item_id: String,
    pub item_name: String,
    pub created_at: String,
    pub expires_at: Option<String>,
    pub access_count: i64,
    /// Whether opening the link also needs a password.
    pub protected: bool,
    /// Only present on the response that creates the share: the token
    /// itself is never stored and cannot be shown again.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token: Option<String>,
}

fn rfc3339(value: OffsetDateTime) -> String {
    value
        .format(&time::format_description::well_known::Rfc3339)
        .unwrap_or_default()
}

/// How long a link lives, as the request asked for it.
///
/// Shared by both kinds of share so a link to an album cannot outlive
/// the rules a link to a file follows.
fn expiry(days: Option<i64>) -> Result<Option<OffsetDateTime>, ApiError> {
    match days {
        None => Ok(None),
        Some(days) if (1..=MAX_EXPIRY_DAYS).contains(&days) => {
            Ok(Some(OffsetDateTime::now_utc() + Duration::days(days)))
        }
        Some(_) => Err(ApiError::bad_request(format!(
            "Choose an expiry between 1 and {MAX_EXPIRY_DAYS} days."
        ))),
    }
}

/// Hashes a link password, if there is one.
///
/// A share password is a shared secret, not a personal one, so it has
/// its own shorter floor: a long passphrase nobody will type from a
/// message is a password nobody uses.
async fn password_hash(password: Option<&str>) -> Result<Option<String>, ApiError> {
    match password.map(str::trim) {
        None | Some("") => Ok(None),
        Some(password) if password.chars().count() < MIN_SHARE_PASSWORD_LENGTH => {
            Err(ApiError::bad_request(format!(
                "A link password needs at least {MIN_SHARE_PASSWORD_LENGTH} characters."
            )))
        }
        Some(password) => Ok(Some(
            homecloud_auth::hash_recovery_code(password.to_owned())
                .await
                .map_err(|_| ApiError::internal())?,
        )),
    }
}

/// `POST /api/v1/items/{item}/shares`
pub async fn create(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Path(item): Path<String>,
    Json(request): Json<CreateShareRequest>,
) -> Result<Json<ShareView>, ApiError> {
    let item_id = parse_item(&item)?;
    let item = repository::item_for_user(state.db(), user, item_id)
        .await
        .map_err(catalog_error)?;

    if item.trashed_at.is_some() {
        return Err(ApiError::conflict("Restore this item before sharing it."));
    }

    let expires_at = expiry(request.expires_in_days)?;
    let password_hash = password_hash(request.password.as_deref()).await?;

    let token = generate_token()?;

    let row: (uuid::Uuid, OffsetDateTime) = sqlx::query_as(
        "INSERT INTO shares (library_id, item_id, created_by, token_hash, expires_at, password_hash)
         VALUES ($1, $2, $3, $4, $5, $6)
         RETURNING id, created_at",
    )
    .bind(item.library.as_uuid())
    .bind(item.id.as_uuid())
    .bind(user.as_uuid())
    .bind(token_hash(token.expose()))
    .bind(expires_at)
    .bind(&password_hash)
    .fetch_one(state.db())
    .await
    .map_err(|error| {
        tracing::error!(error = %error, "share creation failed");
        ApiError::internal()
    })?;

    tracing::info!(item = %item.id, "share link created");

    Ok(Json(ShareView {
        id: row.0.to_string(),
        item_id: item.id.to_string(),
        item_name: item.name,
        created_at: rfc3339(row.1),
        expires_at: expires_at.map(rfc3339),
        access_count: 0,
        protected: password_hash.is_some(),
        token: Some(token.expose().to_owned()),
    }))
}

/// `GET /api/v1/items/{item}/shares`
pub async fn list_for_item(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Path(item): Path<String>,
) -> Result<Json<Vec<ShareView>>, ApiError> {
    let item_id = parse_item(&item)?;
    let item = repository::item_for_user(state.db(), user, item_id)
        .await
        .map_err(catalog_error)?;

    let rows: Vec<(
        uuid::Uuid,
        OffsetDateTime,
        Option<OffsetDateTime>,
        i64,
        bool,
    )> = sqlx::query_as(
        "SELECT id, created_at, expires_at, access_count, password_hash IS NOT NULL
         FROM shares
         WHERE item_id = $1 AND revoked_at IS NULL
         ORDER BY created_at DESC",
    )
    .bind(item.id.as_uuid())
    .fetch_all(state.db())
    .await
    .map_err(|error| {
        tracing::warn!(error = %error, "share listing failed");
        ApiError::dependency_unavailable("database")
    })?;

    Ok(Json(
        rows.into_iter()
            .map(
                |(id, created_at, expires_at, access_count, protected)| ShareView {
                    id: id.to_string(),
                    item_id: item.id.to_string(),
                    item_name: item.name.clone(),
                    created_at: rfc3339(created_at),
                    expires_at: expires_at.map(rfc3339),
                    access_count,
                    protected,
                    token: None,
                },
            )
            .collect(),
    ))
}

/// `DELETE /api/v1/shares/{share}` — revocation takes effect at once.
pub async fn revoke(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Path(share): Path<String>,
) -> Result<StatusCode, ApiError> {
    let share = uuid::Uuid::parse_str(&share).map_err(|_| ApiError::not_found())?;

    // Membership is part of the statement: a share belonging to someone
    // else's library must not even be revocable.
    let affected = sqlx::query(
        "UPDATE shares s
         SET revoked_at = now()
         WHERE s.id = $1
           AND s.revoked_at IS NULL
           AND EXISTS (
               SELECT 1 FROM library_members m
               WHERE m.library_id = s.library_id AND m.user_id = $2
           )",
    )
    .bind(share)
    .bind(user.as_uuid())
    .execute(state.db())
    .await
    .map_err(|error| {
        tracing::warn!(error = %error, "share revocation failed");
        ApiError::dependency_unavailable("database")
    })?
    .rows_affected();

    if affected == 0 {
        return Err(ApiError::not_found());
    }

    Ok(StatusCode::NO_CONTENT)
}

/// `GET /api/v1/libraries/{library}/shares` — every live link in the
/// library, so an owner can audit what is public.
pub async fn list_for_library(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Path(library): Path<String>,
) -> Result<Json<Vec<ShareView>>, ApiError> {
    let library = crate::library::parse_library(&library)?;
    authorize(&state, user, library).await?;

    /// One row of the library-wide share listing.
    type LibraryShareRow = (
        uuid::Uuid,
        uuid::Uuid,
        String,
        OffsetDateTime,
        Option<OffsetDateTime>,
        i64,
        bool,
    );

    let rows: Vec<LibraryShareRow> = sqlx::query_as(
        "SELECT s.id, s.item_id, i.name, s.created_at, s.expires_at, s.access_count,
                    s.password_hash IS NOT NULL
             FROM shares s
             JOIN items i ON i.id = s.item_id
             WHERE s.library_id = $1 AND s.revoked_at IS NULL
             ORDER BY s.created_at DESC",
    )
    .bind(library.as_uuid())
    .fetch_all(state.db())
    .await
    .map_err(|error| {
        tracing::warn!(error = %error, "share listing failed");
        ApiError::dependency_unavailable("database")
    })?;

    Ok(Json(
        rows.into_iter()
            .map(
                |(id, item_id, item_name, created_at, expires_at, access_count, protected)| {
                    ShareView {
                        id: id.to_string(),
                        item_id: item_id.to_string(),
                        item_name,
                        created_at: rfc3339(created_at),
                        expires_at: expires_at.map(rfc3339),
                        access_count,
                        protected,
                        token: None,
                    }
                },
            )
            .collect(),
    ))
}

// --- Public access. No session; the token is the whole credential. ---

/// What a share token resolves to.
/// One resolved share, as the database returns it: the share, its
/// library, the item or album it names, who made it, and whether it is
/// password-protected.
type ShareRow = (
    uuid::Uuid,
    uuid::Uuid,
    Option<uuid::Uuid>,
    Option<uuid::Uuid>,
    uuid::Uuid,
    bool,
);

/// What a link points at. Exactly one of the two, because a share of
/// both, or of neither, is not a thing the reader knows how to answer.
#[derive(Debug, Clone, Copy)]
enum Target {
    /// A file, or a folder and everything under it.
    Item(ItemId),
    /// An album: a set someone arranged, which owns no bytes and has no
    /// path. Sharing the folder its pictures happen to sit in would
    /// share whatever else is in that folder.
    Album(uuid::Uuid),
}

struct Capability {
    library: LibraryId,
    target: Target,
    #[allow(dead_code)]
    created_by: UserId,
}

/// Resolves a token, or reports the same "not found" for a token that is
/// unknown, expired, or revoked. A visitor must not be able to tell
/// which, because that would confirm that a link once existed.
async fn resolve(
    state: &AppState,
    token: &str,
    unlock: Option<&str>,
) -> Result<Capability, ApiError> {
    let pool = state.db();

    // Bounded before it reaches the database: a token is a fixed size,
    // and an enormous string is not worth hashing.
    if !token::is_plausible(token) {
        return Err(ApiError::not_found());
    }

    let row: Option<ShareRow> = sqlx::query_as(
        "UPDATE shares
         SET access_count = access_count + 1, last_accessed_at = now()
         WHERE token_hash = $1
           AND revoked_at IS NULL
           AND (expires_at IS NULL OR expires_at > now())
         RETURNING id, library_id, item_id, album_id, created_by, password_hash IS NOT NULL",
    )
    .bind(token_hash(token))
    .fetch_optional(pool)
    .await
    .map_err(|error| {
        tracing::warn!(error = %error, "share lookup failed");
        ApiError::dependency_unavailable("database")
    })?;

    let Some((id, library, item, album, created_by, protected)) = row else {
        return Err(ApiError::not_found());
    };

    let target = match (item, album) {
        (Some(item), None) => Target::Item(ItemId::from_uuid(item)),
        (None, Some(album)) => Target::Album(album),
        // The database constraint makes this unreachable; treating it as
        // "not found" rather than unwrapping keeps a corrupt row from
        // taking a request down.
        _ => return Err(ApiError::not_found()),
    };

    // A protected link discloses nothing — not even the item's name —
    // until the password has been proved.
    if protected && !unlock.is_some_and(|key| state.share_unlocks().permits(key, id)) {
        return Err(ApiError::password_required());
    }

    Ok(Capability {
        library: LibraryId::from_uuid(library),
        target,
        created_by: UserId::from_uuid(created_by),
    })
}

#[derive(Debug, Deserialize)]
pub struct UnlockRequest {
    pub password: String,
}

#[derive(Debug, Serialize)]
pub struct UnlockResponse {
    /// Opaque key to pass back as `key=` on subsequent requests. Not the
    /// password, and good for an hour.
    pub key: String,
}

/// `POST /api/v1/public/{token}/unlock`
///
/// Proves the password on a protected link. Attempts are throttled per
/// link, so a password sent in a message cannot be guessed at speed.
pub async fn unlock(
    State(state): State<AppState>,
    Path(token): Path<String>,
    Json(request): Json<UnlockRequest>,
) -> Result<Json<UnlockResponse>, ApiError> {
    if !token::is_plausible(&token) {
        return Err(ApiError::not_found());
    }

    let row: Option<(uuid::Uuid, Option<String>)> = sqlx::query_as(
        "SELECT id, password_hash FROM shares
         WHERE token_hash = $1
           AND revoked_at IS NULL
           AND (expires_at IS NULL OR expires_at > now())",
    )
    .bind(token_hash(&token))
    .fetch_optional(state.db())
    .await
    .map_err(|error| {
        tracing::warn!(error = %error, "share lookup failed");
        ApiError::dependency_unavailable("database")
    })?;

    let Some((id, Some(stored_hash))) = row else {
        // Unknown link, or one with no password: the same answer either
        // way, so this endpoint cannot be used to enumerate links.
        return Err(ApiError::not_found());
    };

    let throttle_key = format!("share:{id}");
    if let Err(retry_after) = state.login_attempts().check(&throttle_key) {
        return Err(ApiError::too_many_requests(retry_after));
    }

    if !homecloud_auth::verify_recovery_code(request.password, stored_hash).await {
        state.login_attempts().record_failure(&throttle_key);
        return Err(ApiError::unauthorized("That password does not match."));
    }

    state.login_attempts().record_success(&throttle_key);

    Ok(Json(UnlockResponse {
        key: state.share_unlocks().issue(id)?,
    }))
}

/// Rewrites an item's path to be relative to the shared root.
///
/// Without this a link would hand over the folder's position in someone
/// else's library — "Photos/2019/Wedding" — which the recipient was
/// never given. What they get is the path they can see: the shared item
/// itself is empty, and a child is named relative to it.
fn relative_to(root: &Item, mut view: ItemView) -> ItemView {
    let root_path = root.path.to_string();

    view.path = view
        .path
        .strip_prefix(&root_path)
        .map(|rest| rest.trim_start_matches('/').to_owned())
        .unwrap_or_default();

    view
}

/// Loads an item and proves it is the shared item or lives inside it.
///
/// This is the containment rule for shares: a token for one folder can
/// never be pointed at something outside that folder, whatever id the
/// caller supplies.
async fn item_within(
    pool: &PgPool,
    capability: &Capability,
    requested: Option<ItemId>,
) -> Result<(Item, Item), ApiError> {
    let root_id = match capability.target {
        Target::Item(item) => item,
        // An album has no root item to be inside of; membership is the
        // containment rule instead.
        Target::Album(album) => return album_member(pool, capability, album, requested).await,
    };

    let root: Item = repository::item_in_library(pool, capability.library, root_id)
        .await
        .map_err(catalog_error)?;

    if root.trashed_at.is_some() || root.missing_since.is_some() {
        return Err(ApiError::not_found());
    }

    let Some(requested) = requested else {
        return Ok((root.clone(), root));
    };

    let item = repository::item_in_library(pool, capability.library, requested)
        .await
        .map_err(catalog_error)?;

    let inside = item.id == root.id
        || item
            .path
            .to_string()
            .starts_with(&format!("{}/", root.path));

    if !inside || item.trashed_at.is_some() || item.missing_since.is_some() {
        return Err(ApiError::not_found());
    }

    Ok((root, item))
}

/// The containment rule for an album share: a picture is reachable if it
/// is in the album, and nothing else is.
///
/// Returns the requested picture as both root and item so the caller's
/// relative-path logic keeps working; an album has no path to be
/// relative to.
async fn album_member(
    pool: &PgPool,
    capability: &Capability,
    album: uuid::Uuid,
    requested: Option<ItemId>,
) -> Result<(Item, Item), ApiError> {
    let Some(requested) = requested else {
        return Err(ApiError::not_found());
    };

    let member: Option<bool> =
        sqlx::query_scalar("SELECT true FROM album_items WHERE album_id = $1 AND item_id = $2")
            .bind(album)
            .bind(requested.as_uuid())
            .fetch_optional(pool)
            .await
            .map_err(|error| {
                tracing::warn!(error = %error, "album membership lookup failed");
                ApiError::dependency_unavailable("database")
            })?;

    if member.is_none() {
        return Err(ApiError::not_found());
    }

    let item = repository::item_in_library(pool, capability.library, requested)
        .await
        .map_err(catalog_error)?;

    if item.trashed_at.is_some() || item.missing_since.is_some() {
        return Err(ApiError::not_found());
    }

    Ok((item.clone(), item))
}

#[derive(Debug, Serialize)]
pub struct PublicShareView {
    /// The shared item itself. Absent when the link points at an album,
    /// which is not an item.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub album: Option<PublicAlbumView>,
    /// The shared item itself.
    pub item: ItemView,
    /// Children, when the shared item is a folder.
    pub items: Vec<ItemView>,
    /// Path relative to the shared root, for a breadcrumb that cannot
    /// reveal where the folder sits in the library.
    pub relative_path: String,
}

#[derive(Debug, Deserialize)]
pub struct PublicQuery {
    /// Item inside the shared folder to look at. Absent means the shared
    /// item itself.
    pub item: Option<String>,
    /// Unlock key for a password-protected link. A query parameter
    /// because `<img>` and download links cannot send headers; the
    /// request log records paths only, never query strings.
    pub key: Option<String>,
}

/// What a visitor is told about a shared album: its name, and the
/// pictures in it. Never who made it or which library it belongs to.
#[derive(Debug, Serialize)]
pub struct PublicAlbumView {
    pub name: String,
    pub item_count: i64,
}

/// `GET /api/v1/public/{token}` — what this link points at.
pub async fn public_view(
    State(state): State<AppState>,
    Path(token): Path<String>,
    Query(query): Query<PublicQuery>,
) -> Result<Json<PublicShareView>, ApiError> {
    let capability = resolve(&state, &token, query.key.as_deref()).await?;

    if let Target::Album(album) = capability.target {
        return public_album(&state, &capability, album).await;
    }

    let requested = query.item.as_deref().map(parse_item).transpose()?;
    let (root, item) = item_within(state.db(), &capability, requested).await?;

    let children = if item.is_folder() {
        repository::children(state.db(), capability.library, Some(item.id))
            .await
            .map_err(catalog_error)?
    } else {
        Vec::new()
    };

    // Relative to the share root: a visitor learns the folder they were
    // given, never where it lives in someone's library.
    let relative_path = item
        .path
        .to_string()
        .strip_prefix(&root.path.to_string())
        .map(|rest| rest.trim_start_matches('/').to_owned())
        .unwrap_or_default();

    Ok(Json(PublicShareView {
        album: None,
        item: relative_to(&root, ItemView::from(&item).without_location()),
        items: crate::view::items(&children)
            .into_iter()
            .map(|child| relative_to(&root, child.without_location()))
            .collect(),
        relative_path,
    }))
}

/// A shared album: the arrangement, in the order it was arranged.
async fn public_album(
    state: &AppState,
    capability: &Capability,
    album: uuid::Uuid,
) -> Result<Json<PublicShareView>, ApiError> {
    let name: Option<String> = sqlx::query_scalar("SELECT name FROM albums WHERE id = $1")
        .bind(album)
        .fetch_optional(state.db())
        .await
        .map_err(|error| {
            tracing::warn!(error = %error, "shared album lookup failed");
            ApiError::dependency_unavailable("database")
        })?;

    let Some(name) = name else {
        return Err(ApiError::not_found());
    };

    let ids: Vec<uuid::Uuid> = sqlx::query_scalar(
        "SELECT ai.item_id FROM album_items ai
         JOIN items i ON i.id = ai.item_id
         WHERE ai.album_id = $1
           AND i.trashed_at IS NULL
           AND i.missing_since IS NULL
         ORDER BY ai.position",
    )
    .bind(album)
    .fetch_all(state.db())
    .await
    .map_err(|error| {
        tracing::warn!(error = %error, "shared album contents lookup failed");
        ApiError::dependency_unavailable("database")
    })?;

    let mut items = Vec::with_capacity(ids.len());
    for id in ids {
        if let Ok(item) =
            repository::item_in_library(state.db(), capability.library, ItemId::from_uuid(id)).await
        {
            items.push(item);
        }
    }

    // The album stands in for the item a file share would name, so the
    // shape of the response does not change for one kind of link.
    let cover = items.first().cloned();

    Ok(Json(PublicShareView {
        album: Some(PublicAlbumView {
            name: name.clone(),
            item_count: items.len() as i64,
        }),
        item: match cover {
            Some(item) => {
                let mut view = ItemView::from(&item).without_location();
                // A visitor sees the album's name, never where its
                // pictures sit in somebody's library.
                view.path = view.name.clone();
                view
            }
            None => ItemView::empty_album(&name),
        },
        items: items
            .iter()
            .map(|item| {
                let mut view = ItemView::from(item).without_location();
                view.path = view.name.clone();
                view
            })
            .collect(),
        relative_path: String::new(),
    }))
}

/// `POST /api/v1/albums/{album}/shares` — share an album.
pub async fn create_for_album(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Path(album): Path<String>,
    Json(request): Json<CreateShareRequest>,
) -> Result<Json<ShareView>, ApiError> {
    let album = uuid::Uuid::parse_str(&album).map_err(|_| ApiError::not_found())?;

    // Membership is the check: an album belongs to a library, and only
    // its members know it exists.
    let row: Option<(uuid::Uuid, String)> = sqlx::query_as(
        "SELECT a.library_id, a.name FROM albums a
         WHERE a.id = $1
           AND EXISTS (
               SELECT 1 FROM library_members m
               WHERE m.library_id = a.library_id AND m.user_id = $2
           )",
    )
    .bind(album)
    .bind(user.as_uuid())
    .fetch_optional(state.db())
    .await
    .map_err(|error| {
        tracing::warn!(error = %error, "album lookup failed");
        ApiError::dependency_unavailable("database")
    })?;

    let Some((library, name)) = row else {
        return Err(ApiError::not_found());
    };

    let expires_at = expiry(request.expires_in_days)?;
    let password_hash = password_hash(request.password.as_deref()).await?;
    let token = generate_token()?;

    let row: (uuid::Uuid, OffsetDateTime) = sqlx::query_as(
        "INSERT INTO shares (library_id, album_id, created_by, token_hash, expires_at, password_hash)
         VALUES ($1, $2, $3, $4, $5, $6)
         RETURNING id, created_at",
    )
    .bind(library)
    .bind(album)
    .bind(user.as_uuid())
    .bind(token_hash(token.expose()))
    .bind(expires_at)
    .bind(password_hash.as_deref())
    .fetch_one(state.db())
    .await
    .map_err(|error| {
        tracing::error!(error = %error, "could not create an album share");
        ApiError::internal()
    })?;

    tracing::info!("an album was shared");

    Ok(Json(ShareView {
        id: row.0.to_string(),
        item_id: album.to_string(),
        item_name: name,
        created_at: rfc3339(row.1),
        expires_at: expires_at.map(rfc3339),
        access_count: 0,
        protected: password_hash.is_some(),
        token: Some(token.expose().to_owned()),
    }))
}

/// `GET /api/v1/public/{token}/content`
pub async fn public_content(
    State(state): State<AppState>,
    Path(token): Path<String>,
    Query(query): Query<PublicQuery>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    let capability = resolve(&state, &token, query.key.as_deref()).await?;
    let requested = query.item.as_deref().map(parse_item).transpose()?;
    let (_, item) = item_within(state.db(), &capability, requested).await?;

    if item.is_folder() {
        return Err(ApiError::bad_request(
            "A folder has no content to download.",
        ));
    }

    let storage = storage_for(&state, capability.library).await?;

    crate::transfers::stream_file(&storage, &item, &headers).await
}

/// `GET /api/v1/public/{token}/thumbnail`
pub async fn public_thumbnail(
    State(state): State<AppState>,
    Path(token): Path<String>,
    Query(query): Query<PublicQuery>,
) -> Result<Response, ApiError> {
    let capability = resolve(&state, &token, query.key.as_deref()).await?;
    let requested = query.item.as_deref().map(parse_item).transpose()?;
    let (_, item) = item_within(state.db(), &capability, requested).await?;

    if !is_thumbnailable(item.content_type.as_deref()) {
        return Err(ApiError::bad_request(
            "This item does not have a picture preview.",
        ));
    }

    let storage = storage_for(&state, capability.library).await?;

    crate::thumbnails::render(&storage, &item, ThumbnailSize::Small).await
}

/// Removes shares whose expiry has passed. Correctness does not depend
/// on it — expiry is enforced on every lookup — but an owner's list of
/// live links should not fill with dead ones.
pub async fn purge_expired(pool: &PgPool) -> Result<u64, sqlx::Error> {
    let result = sqlx::query(
        "UPDATE shares SET revoked_at = now()
         WHERE revoked_at IS NULL AND expires_at IS NOT NULL AND expires_at <= now()",
    )
    .execute(pool)
    .await?;

    Ok(result.rows_affected())
}
