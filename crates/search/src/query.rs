//! Searching.
//!
//! One query ranks name matches and content matches together, because a
//! person looking for "invoice" does not care whether the word is in the
//! file name or on page two.

use homecloud_domain::identity::{ItemId, LibraryId};
use sqlx::{PgPool, Row};

use crate::index::IndexError;

/// Where a result matched.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MatchKind {
    Name,
    Content,
    /// Both, which is a strong signal and ranks first.
    NameAndContent,
}

/// One search result: the item, plus why it matched.
#[derive(Debug, Clone)]
pub struct Hit {
    pub item: ItemId,
    pub kind: MatchKind,
    /// A short passage around the match, for content hits.
    pub snippet: Option<String>,
}

/// Longest result set a caller can ask for.
pub const MAX_RESULTS: i64 = 200;

/// Searches names and document text.
///
/// The query is a user-supplied string, bound as a parameter and handed
/// to `websearch_to_tsquery`, which accepts anything a person might type
/// — including quotes, `or`, and punctuation — without failing.
pub async fn search(
    pool: &PgPool,
    library: LibraryId,
    query: &str,
    limit: i64,
) -> Result<Vec<Hit>, IndexError> {
    let limit = limit.clamp(1, MAX_RESULTS);

    let rows = sqlx::query(
        "WITH q AS (SELECT websearch_to_tsquery('simple', $2) AS tsquery)
         SELECT
             i.id,
             (i.name ILIKE '%' || $2 || '%'
                 OR to_tsvector('simple', replace(i.name, '.', ' ')) @@ q.tsquery) AS name_match,
             (t.search_vector @@ q.tsquery) AS content_match,
             CASE
                 WHEN t.search_vector @@ q.tsquery
                 THEN ts_headline('simple', t.content, q.tsquery,
                                  'MaxWords=28, MinWords=10, MaxFragments=1, StartSel=<<, StopSel=>>')
                 ELSE NULL
             END AS snippet,
             ts_rank(coalesce(t.search_vector, ''::tsvector), q.tsquery) AS content_rank
         FROM items i
         CROSS JOIN q
         LEFT JOIN item_text t ON t.item_id = i.id
         WHERE i.library_id = $1
           AND i.trashed_at IS NULL
           AND i.missing_since IS NULL
           AND (
             i.name ILIKE '%' || $2 || '%'
             OR to_tsvector('simple', replace(i.name, '.', ' ')) @@ q.tsquery
             OR t.search_vector @@ q.tsquery
           )
         ORDER BY
             (i.name ILIKE '%' || $2 || '%') DESC,
             content_rank DESC,
             (i.kind = 'folder') DESC,
             lower(i.name)
         LIMIT $3",
    )
    .bind(library.as_uuid())
    .bind(query)
    .bind(limit)
    .fetch_all(pool)
    .await?;

    rows.iter()
        .map(|row| {
            let name_match: bool = row
                .try_get::<Option<bool>, _>("name_match")?
                .unwrap_or(false);
            let content_match: bool = row
                .try_get::<Option<bool>, _>("content_match")?
                .unwrap_or(false);

            Ok(Hit {
                item: ItemId::from_uuid(row.try_get("id")?),
                kind: match (name_match, content_match) {
                    (true, true) => MatchKind::NameAndContent,
                    (false, true) => MatchKind::Content,
                    _ => MatchKind::Name,
                },
                snippet: row.try_get("snippet")?,
            })
        })
        .collect()
}
