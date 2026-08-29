//! Library-relative paths.
//!
//! Every path that reaches a storage backend comes from a client, a
//! filename, or a database row, and is therefore untrusted. Validation
//! happens once, here, so no backend has to re-derive the rules.

use std::fmt;
use std::path::{Component, Path, PathBuf};

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum PathError {
    #[error("path must be relative to the library root")]
    NotRelative,
    #[error("path must not traverse outside the library root")]
    Traversal,
    #[error("path must not contain a null byte")]
    NullByte,
    #[error("path segment is not valid UTF-8")]
    NotUtf8,
    #[error("path is too long")]
    TooLong,
}

/// Longest accepted relative path. Bounds work done per request and stays
/// well inside typical filesystem limits.
const MAX_PATH_LEN: usize = 4096;

/// A validated path relative to a library root. Constructing one is the
/// only way to name a location inside a library.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct LibraryPath(PathBuf);

impl LibraryPath {
    /// The library root itself.
    pub fn root() -> Self {
        Self(PathBuf::new())
    }

    /// Validates untrusted input. Accepts `/`-separated segments; `.` and
    /// empty segments are dropped; `..`, absolute paths, Windows-style
    /// prefixes, and null bytes are rejected outright rather than
    /// normalised, because silently rewriting a hostile path hides the
    /// attempt.
    pub fn parse(raw: &str) -> Result<Self, PathError> {
        if raw.len() > MAX_PATH_LEN {
            return Err(PathError::TooLong);
        }
        if raw.contains('\0') {
            return Err(PathError::NullByte);
        }

        let mut normalised = PathBuf::new();
        for component in Path::new(raw).components() {
            match component {
                Component::Normal(segment) => {
                    let segment = segment.to_str().ok_or(PathError::NotUtf8)?;
                    normalised.push(segment);
                }
                Component::CurDir => {}
                Component::ParentDir => return Err(PathError::Traversal),
                Component::RootDir | Component::Prefix(_) => return Err(PathError::NotRelative),
            }
        }

        Ok(Self(normalised))
    }

    pub fn is_root(&self) -> bool {
        self.0.as_os_str().is_empty()
    }

    pub fn as_path(&self) -> &Path {
        &self.0
    }

    /// Appends a single already-known-good segment, such as a directory
    /// entry name read back from the filesystem.
    pub fn join_segment(&self, segment: &str) -> Result<Self, PathError> {
        let mut child = self.clone();
        child.0.push(segment);
        Self::parse(child.0.to_str().ok_or(PathError::NotUtf8)?)
    }
}

impl fmt::Display for LibraryPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0.to_string_lossy())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_relative_paths_are_accepted() {
        let path = LibraryPath::parse("photos/2024/img.jpg").expect("valid path");

        assert_eq!(path.as_path(), Path::new("photos/2024/img.jpg"));
        assert!(!path.is_root());
    }

    #[test]
    fn an_empty_path_is_the_root() {
        assert!(LibraryPath::parse("").expect("valid").is_root());
        assert!(LibraryPath::parse("./").expect("valid").is_root());
    }

    #[test]
    fn traversal_is_rejected() {
        for hostile in ["..", "../etc/passwd", "photos/../../etc/passwd", "a/b/.."] {
            assert_eq!(
                LibraryPath::parse(hostile),
                Err(PathError::Traversal),
                "accepted `{hostile}`"
            );
        }
    }

    #[test]
    fn absolute_paths_are_rejected() {
        assert_eq!(
            LibraryPath::parse("/etc/passwd"),
            Err(PathError::NotRelative)
        );
    }

    #[test]
    fn null_bytes_are_rejected() {
        assert_eq!(
            LibraryPath::parse("photo.jpg\0.txt"),
            Err(PathError::NullByte)
        );
    }

    #[test]
    fn overlong_paths_are_rejected() {
        assert_eq!(
            LibraryPath::parse(&"a".repeat(MAX_PATH_LEN + 1)),
            Err(PathError::TooLong)
        );
    }

    #[test]
    fn unicode_names_survive_unchanged() {
        let raw = "写真/naïve/🌅.jpg";

        assert_eq!(LibraryPath::parse(raw).expect("valid").to_string(), raw);
    }

    #[test]
    fn redundant_separators_and_dots_are_dropped() {
        let path = LibraryPath::parse("photos/./2024//img.jpg").expect("valid");

        assert_eq!(path.as_path(), Path::new("photos/2024/img.jpg"));
    }
}
