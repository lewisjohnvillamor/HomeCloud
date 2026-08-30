//! Library-scoped routes: listing, browsing, scanning, and search.

use axum::extract::{Path, Query, State};
use axum::Json;
use homecloud_catalog::repository::{self, CatalogError};
use homecloud_catalog::scan;
use homecloud_domain::identity::{ItemId, LibraryId};
use homecloud_storage::{FilesystemStorage, LibraryPath};
use serde::{Deserialize, Serialize};

use crate::app::AppState;
use crate::auth::CurrentUser;
use crate::error::ApiError;
use crate::view::{self, ItemView, SearchResultView};

/// Largest page a client may ask for. Bounds the work one request can
/// cause regardless of what the client sends.
const MAX_PAGE_SIZE: i64 = 500;
const DEFAULT_PAGE_SIZE: i64 = 200;

#[derive(Debug, Serialize)]
pub struct LibraryView {
    pub id: String,
    pub name: String,
    pub role: String,
    pub root_path: Option<String>,
}

/// `GET /api/v1/libraries`
pub async fn list(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
) -> Result<Json<Vec<LibraryView>>, ApiError> {
    let libraries = repository::libraries_for_user(state.db(), user)
        .await
        .map_err(catalog_error)?;

    Ok(Json(
        libraries
            .into_iter()
            .map(|library| LibraryView {
                id: library.id.to_string(),
                name: library.name,
                role: library.role,
                root_path: library.root_path,
            })
            .collect(),
    ))
}

#[derive(Debug, Deserialize)]
pub struct BrowseQuery {
    /// Library-relative folder path. Absent or empty means the root.
    #[serde(default)]
    pub path: String,
}

#[derive(Debug, Serialize)]
pub struct BrowseResponse {
    /// The folder being listed; absent for the library root.
    pub folder: Option<ItemView>,
    /// Ancestors from the root down to the folder, for a breadcrumb.
    pub breadcrumb: Vec<Breadcrumb>,
    pub items: Vec<ItemView>,
}

#[derive(Debug, Serialize)]
pub struct Breadcrumb {
    pub name: String,
    pub path: String,
}

/// `GET /api/v1/libraries/{library}/browse?path=...`
///
/// Browsing by path is what a file UI actually needs; items are still
/// addressed by id everywhere else.
pub async fn browse(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Path(library): Path<String>,
    Query(query): Query<BrowseQuery>,
) -> Result<Json<BrowseResponse>, ApiError> {
    let library = parse_library(&library)?;
    authorize(&state, user, library).await?;

    let path = LibraryPath::parse(&query.path)
        .map_err(|error| ApiError::bad_request(error.to_string()))?;

    let folder = if path.is_root() {
        None
    } else {
        Some(
            repository::item_at_path(state.db(), library, &path)
                .await
                .map_err(catalog_error)?,
        )
    };

    let children = repository::children(state.db(), library, folder.as_ref().map(|item| item.id))
        .await
        .map_err(catalog_error)?;

    Ok(Json(BrowseResponse {
        folder: folder.as_ref().map(ItemView::from),
        breadcrumb: breadcrumb_for(&path),
        items: view::items(&children),
    }))
}

#[derive(Debug, Deserialize)]
pub struct PageQuery {
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

impl PageQuery {
    fn limit(&self) -> i64 {
        self.limit
            .unwrap_or(DEFAULT_PAGE_SIZE)
            .clamp(1, MAX_PAGE_SIZE)
    }

    fn offset(&self) -> i64 {
        self.offset.unwrap_or(0).max(0)
    }
}

/// `GET /api/v1/libraries/{library}/photos`
pub async fn photos(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Path(library): Path<String>,
    Query(page): Query<PageQuery>,
) -> Result<Json<Vec<ItemView>>, ApiError> {
    let library = parse_library(&library)?;
    authorize(&state, user, library).await?;

    let items = repository::visual_media(state.db(), library, page.limit(), page.offset())
        .await
        .map_err(catalog_error)?;

    Ok(Json(view::items(&items)))
}

#[derive(Debug, Deserialize)]
pub struct SearchQuery {
    pub q: String,
    pub limit: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct MemoryGroup {
    pub title: String,
    pub subtitle: String,
    pub items: Vec<ItemView>,
}

/// `GET /api/v1/libraries/{library}/memories`
///
/// Deterministic collections for the TV and the home screen: what was
/// photographed on this day in earlier years, and what arrived recently.
/// No model is involved, so this works with AI disabled — which is the
/// point.
pub async fn memories(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Path(library): Path<String>,
) -> Result<Json<Vec<MemoryGroup>>, ApiError> {
    let library = parse_library(&library)?;
    authorize(&state, user, library).await?;

    Ok(Json(memory_groups(&state, library).await?))
}

/// The memory groups themselves, without an opinion about who is asking.
///
/// A television reaches these through its own capability rather than a
/// session, so the collection logic lives here and each caller proves
/// its own right to the library first.
pub async fn memory_groups(
    state: &AppState,
    library: LibraryId,
) -> Result<Vec<MemoryGroup>, ApiError> {
    let today = time::OffsetDateTime::now_utc();
    let mut groups = Vec::new();

    let on_this_day = repository::on_this_day(state.db(), library, today, DEFAULT_PAGE_SIZE)
        .await
        .map_err(catalog_error)?;
    if !on_this_day.is_empty() {
        let years: Vec<String> = {
            let mut years: Vec<i32> = on_this_day
                .iter()
                .filter_map(|item| item.modified_at.map(|value| value.year()))
                .collect();
            years.sort_unstable();
            years.dedup();
            years.iter().rev().map(|year| year.to_string()).collect()
        };

        groups.push(MemoryGroup {
            title: "On this day".to_owned(),
            subtitle: years.join(" · "),
            items: view::items(&on_this_day),
        });
    }

    let recent = repository::visual_media(state.db(), library, DEFAULT_PAGE_SIZE, 0)
        .await
        .map_err(catalog_error)?;
    if !recent.is_empty() {
        groups.push(MemoryGroup {
            title: "Recently added".to_owned(),
            subtitle: format!("{} photos", recent.len()),
            items: view::items(&recent),
        });
    }

    Ok(groups)
}

/// `GET /api/v1/libraries/{library}/search?q=...`
///
/// Matches file names and the text inside documents, ranked together: a
/// person looking for "invoice" does not care which one it was found in.
pub async fn search(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Path(library): Path<String>,
    Query(query): Query<SearchQuery>,
) -> Result<Json<Vec<SearchResultView>>, ApiError> {
    let library = parse_library(&library)?;
    authorize(&state, user, library).await?;

    let term = query.q.trim();
    if term.is_empty() {
        return Ok(Json(Vec::new()));
    }

    let limit = query
        .limit
        .unwrap_or(DEFAULT_PAGE_SIZE)
        .clamp(1, MAX_PAGE_SIZE);
    let hits = homecloud_search::query::search(state.db(), library, term, limit)
        .await
        .map_err(|error| {
            tracing::warn!(error = %error, "search failed");
            ApiError::dependency_unavailable("database")
        })?;

    // The ranking happened in the query that produced these ids; loading
    // the items must not reorder them.
    let ids: Vec<_> = hits.iter().map(|hit| hit.item).collect();
    let items = repository::items_by_ids(state.db(), library, &ids)
        .await
        .map_err(catalog_error)?;

    Ok(Json(
        items
            .iter()
            .zip(hits.iter())
            .map(|(item, hit)| SearchResultView {
                item: ItemView::from(item),
                matched: match hit.kind {
                    homecloud_search::MatchKind::Name => "name",
                    homecloud_search::MatchKind::Content => "content",
                    homecloud_search::MatchKind::NameAndContent => "name_and_content",
                },
                snippet: hit.snippet.clone(),
            })
            .collect(),
    ))
}

/// `GET /api/v1/libraries/{library}/trash`
pub async fn trash(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Path(library): Path<String>,
) -> Result<Json<Vec<ItemView>>, ApiError> {
    let library = parse_library(&library)?;
    authorize(&state, user, library).await?;

    let items = repository::trashed(state.db(), library)
        .await
        .map_err(catalog_error)?;

    Ok(Json(view::items(&items)))
}

/// `POST /api/v1/libraries/{library}/scan`
///
/// Scanning walks the whole library, so it runs as a background task:
/// holding an HTTP request open for it would tie a user's browser to the
/// size of their disk.
pub async fn start_scan(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Path(library): Path<String>,
) -> Result<Json<crate::scanjob::ScanStatusView>, ApiError> {
    let library = parse_library(&library)?;
    authorize(&state, user, library).await?;

    let storage = storage_for(&state, library).await?;
    let status = state.scans().start(library, state.db().clone(), storage);

    Ok(Json(status))
}

/// `GET /api/v1/libraries/{library}/scan`
pub async fn scan_status(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Path(library): Path<String>,
) -> Result<Json<crate::scanjob::ScanStatusView>, ApiError> {
    let library = parse_library(&library)?;
    authorize(&state, user, library).await?;

    Ok(Json(state.scans().status(library)))
}

/// Opens the filesystem backend for a library.
///
/// The root comes from the library row when it has one, so a deployment
/// that later gains a second library does not have to share one root.
pub async fn storage_for(
    state: &AppState,
    library: LibraryId,
) -> Result<FilesystemStorage, ApiError> {
    let root: Option<Option<String>> =
        sqlx::query_scalar("SELECT root_path FROM libraries WHERE id = $1")
            .bind(library.as_uuid())
            .fetch_optional(state.db())
            .await
            .map_err(|error| {
                tracing::warn!(error = %error, "library lookup failed");
                ApiError::dependency_unavailable("database")
            })?;

    let root = root
        .flatten()
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| state.storage_root().to_path_buf());

    FilesystemStorage::open(&root).await.map_err(|error| {
        tracing::error!(error = %error, "library root is not usable");
        ApiError::dependency_unavailable("library storage")
    })
}

pub fn parse_library(raw: &str) -> Result<LibraryId, ApiError> {
    uuid::Uuid::parse_str(raw)
        .map(LibraryId::from_uuid)
        .map_err(|_| ApiError::not_found())
}

pub fn parse_item(raw: &str) -> Result<ItemId, ApiError> {
    uuid::Uuid::parse_str(raw)
        .map(ItemId::from_uuid)
        // A malformed id is indistinguishable from an id that does not
        // exist, which is also what a caller probing for items should see.
        .map_err(|_| ApiError::not_found())
}

/// Confirms membership before any library-scoped work happens.
pub async fn authorize(
    state: &AppState,
    user: homecloud_domain::identity::UserId,
    library: LibraryId,
) -> Result<(), ApiError> {
    let member = repository::is_member(state.db(), user, library)
        .await
        .map_err(catalog_error)?;

    if member {
        Ok(())
    } else {
        // Not "forbidden": whether a library exists is itself something
        // a non-member should not learn.
        Err(ApiError::not_found())
    }
}

/// Maps catalog failures to the API's vocabulary without leaking SQL.
pub fn catalog_error(error: CatalogError) -> ApiError {
    match error {
        CatalogError::NotFound => ApiError::not_found(),
        CatalogError::InvalidPath(error) => ApiError::bad_request(error.to_string()),
        CatalogError::Database(error) => {
            tracing::warn!(error = %error, "catalog query failed");
            ApiError::dependency_unavailable("database")
        }
    }
}

fn breadcrumb_for(path: &LibraryPath) -> Vec<Breadcrumb> {
    let mut crumbs = Vec::new();
    let mut so_far = String::new();

    for segment in path.as_path().iter().filter_map(|value| value.to_str()) {
        if so_far.is_empty() {
            so_far.push_str(segment);
        } else {
            so_far.push('/');
            so_far.push_str(segment);
        }

        crumbs.push(Breadcrumb {
            name: segment.to_owned(),
            path: so_far.clone(),
        });
    }

    crumbs
}

/// Re-exported so route handlers elsewhere can reuse the same scan
/// directory names the catalog skips.
pub const RESERVED_DIRECTORIES: [&str; 3] = [
    scan::TRASH_DIRECTORY,
    scan::UPLOAD_DIRECTORY,
    scan::DERIVATIVES_DIRECTORY,
];
