//! Catalogued files and folders.

use homecloud_domain::identity::{ItemId, LibraryId};
use homecloud_storage::LibraryPath;
use serde::Serialize;
use time::OffsetDateTime;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ItemKind {
    File,
    Folder,
}

impl ItemKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            ItemKind::File => "file",
            ItemKind::Folder => "folder",
        }
    }

    pub fn parse(raw: &str) -> Option<Self> {
        match raw {
            "file" => Some(ItemKind::File),
            "folder" => Some(ItemKind::Folder),
            _ => None,
        }
    }
}

/// One catalogued entry.
///
/// `path` is where the item currently lives; `id` is what it *is*. Move
/// or rename an item and the path changes while the id does not, which
/// is what lets links, shares, and offline state survive reorganisation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Item {
    pub id: ItemId,
    pub library: LibraryId,
    pub parent: Option<ItemId>,
    pub path: LibraryPath,
    pub name: String,
    pub kind: ItemKind,
    pub size_bytes: i64,
    pub content_type: Option<String>,
    pub modified_at: Option<OffsetDateTime>,
    /// When the picture was taken, as the camera recorded it. Absent for
    /// anything that is not a photo, and for photos that never said.
    pub taken_at: Option<OffsetDateTime>,
    /// The camera, as one line: "Fujifilm X100V".
    pub camera: Option<String>,
    pub trashed_at: Option<OffsetDateTime>,
    pub missing_since: Option<OffsetDateTime>,
}

impl Item {
    /// The date this item belongs under in a timeline: what the camera
    /// said, or failing that when the file was last written.
    pub fn happened_at(&self) -> Option<OffsetDateTime> {
        self.taken_at.or(self.modified_at)
    }

    pub fn is_folder(&self) -> bool {
        self.kind == ItemKind::Folder
    }

    /// Whether the item is a still image.
    pub fn is_image(&self) -> bool {
        self.content_type
            .as_deref()
            .is_some_and(|value| value.starts_with("image/"))
    }

    /// Whether the item is a video. Videos appear in the Photos timeline
    /// alongside stills, with a poster frame for a thumbnail.
    pub fn is_video(&self) -> bool {
        self.content_type
            .as_deref()
            .is_some_and(|value| value.starts_with("video/"))
    }

    /// Whether the item belongs in a visual timeline at all.
    pub fn is_visual_media(&self) -> bool {
        self.is_image() || self.is_video()
    }
}

/// Guesses a content type from the file name.
///
/// Extension-based and therefore untrusted: it decides how HomeCloud
/// groups an item, never how a browser is told to execute it. Downloads
/// are served with a conservative type of their own.
pub fn guess_content_type(name: &str) -> Option<String> {
    mime_guess::from_path(name)
        .first_raw()
        .map(|value| value.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kinds_round_trip_through_their_stored_form() {
        for kind in [ItemKind::File, ItemKind::Folder] {
            assert_eq!(ItemKind::parse(kind.as_str()), Some(kind));
        }

        assert_eq!(ItemKind::parse("symlink"), None);
    }

    #[test]
    fn content_types_are_guessed_from_the_extension() {
        assert_eq!(
            guess_content_type("photo.JPG").as_deref(),
            Some("image/jpeg")
        );
        assert_eq!(
            guess_content_type("notes.txt").as_deref(),
            Some("text/plain")
        );
        assert_eq!(guess_content_type("mystery"), None);
    }
}
