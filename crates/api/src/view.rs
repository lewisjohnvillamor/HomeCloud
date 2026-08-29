//! Wire representations of catalog objects.
//!
//! Kept separate from the domain types so a change to the database or to
//! internal structure does not silently change the API contract.

use homecloud_catalog::Item;
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct ItemView {
    pub id: String,
    pub name: String,
    pub path: String,
    pub kind: &'static str,
    pub size_bytes: i64,
    pub content_type: Option<String>,
    /// RFC 3339, or absent when the filesystem did not report one.
    pub modified_at: Option<String>,
    /// Whether the Photos view should show this item.
    pub is_image: bool,
    pub trashed: bool,
}

impl From<&Item> for ItemView {
    fn from(item: &Item) -> Self {
        Self {
            id: item.id.to_string(),
            name: item.name.clone(),
            path: item.path.to_string(),
            kind: item.kind.as_str(),
            size_bytes: item.size_bytes,
            content_type: item.content_type.clone(),
            modified_at: item.modified_at.and_then(|value| {
                value
                    .format(&time::format_description::well_known::Rfc3339)
                    .ok()
            }),
            is_image: item.is_image(),
            trashed: item.trashed_at.is_some(),
        }
    }
}

pub fn items(items: &[Item]) -> Vec<ItemView> {
    items.iter().map(ItemView::from).collect()
}
