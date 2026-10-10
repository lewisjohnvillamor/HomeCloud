//! Searching.
//!
//! One query ranks name matches, content matches and recognised objects
//! together, because a person looking for "invoice" does not care whether
//! the word is in the file name or on page two — and a person looking for
//! "dog" does not care whether anyone typed the word at all.

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
    /// Only something a model recognised in the picture. Reported apart
    /// so the interface can present it as an inference, not a fact.
    Objects,
}

/// One search result: the item, plus why it matched.
#[derive(Debug, Clone)]
pub struct Hit {
    pub item: ItemId,
    pub kind: MatchKind,
    /// A short passage around the match, for content hits.
    pub snippet: Option<String>,
    /// What was recognised in the picture, when that is why it matched.
    pub objects: Vec<String>,
}

/// Longest result set a caller can ask for.
pub const MAX_RESULTS: i64 = 200;

/// Searches names, document text, and what was recognised in photographs.
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
             ts_rank(coalesce(t.search_vector, ''::tsvector), q.tsquery) AS content_rank,
             CASE WHEN o.search_vector @@ q.tsquery THEN o.labels END AS objects
         FROM items i
         CROSS JOIN q
         LEFT JOIN item_text t ON t.item_id = i.id
         LEFT JOIN item_objects o ON o.item_id = i.id
         WHERE i.library_id = $1
           AND i.trashed_at IS NULL
           AND i.missing_since IS NULL
           AND (
             i.name ILIKE '%' || $2 || '%'
             OR to_tsvector('simple', replace(i.name, '.', ' ')) @@ q.tsquery
             OR t.search_vector @@ q.tsquery
             OR o.search_vector @@ q.tsquery
           )
         ORDER BY
             (i.name ILIKE '%' || $2 || '%') DESC,
             content_rank DESC,
             -- Something in the picture ranks below words in the file:
             -- a document that mentions a dog was written about one; a
             -- recognised dog is the model's opinion.
             (o.search_vector @@ q.tsquery) DESC NULLS LAST,
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

            let objects: Option<Vec<String>> = row.try_get("objects")?;

            Ok(Hit {
                item: ItemId::from_uuid(row.try_get("id")?),
                kind: match (name_match, content_match, objects.is_some()) {
                    (true, true, _) => MatchKind::NameAndContent,
                    (false, true, _) => MatchKind::Content,
                    (false, false, true) => MatchKind::Objects,
                    _ => MatchKind::Name,
                },
                snippet: row.try_get("snippet")?,
                objects: objects.unwrap_or_default(),
            })
        })
        .collect()
}
