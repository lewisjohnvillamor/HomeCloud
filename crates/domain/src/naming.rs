//! Human-facing names entered by people, validated once at the domain
//! boundary so no other layer has to re-derive the rules.

use std::fmt;

/// Longest accepted name, in Unicode scalar values rather than bytes so
/// the limit means the same thing in every script.
const MAX_NAME_CHARS: usize = 64;

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum NameError {
    #[error("name must not be empty")]
    Empty,
    #[error("name must be at most {MAX_NAME_CHARS} characters")]
    TooLong,
    #[error("name must not contain control characters")]
    ControlCharacter,
}

/// The display name of a library. Names are untrusted input: they are
/// trimmed, length-bounded, and stripped of control characters that would
/// otherwise corrupt logs or terminal output.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LibraryName(String);

impl LibraryName {
    pub fn parse(raw: &str) -> Result<Self, NameError> {
        let trimmed = raw.trim();

        if trimmed.is_empty() {
            return Err(NameError::Empty);
        }
        if trimmed.chars().count() > MAX_NAME_CHARS {
            return Err(NameError::TooLong);
        }
        if trimmed.chars().any(char::is_control) {
            return Err(NameError::ControlCharacter);
        }

        Ok(Self(trimmed.to_owned()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for LibraryName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl From<LibraryName> for String {
    fn from(name: LibraryName) -> Self {
        name.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trims_surrounding_whitespace() {
        let name = LibraryName::parse("  Family Photos \n").expect("valid name");
        assert_eq!(name.as_str(), "Family Photos");
    }

    #[test]
    fn rejects_blank_names() {
        assert_eq!(LibraryName::parse("   "), Err(NameError::Empty));
    }

    #[test]
    fn rejects_control_characters() {
        assert_eq!(
            LibraryName::parse("Home\u{7}Cloud"),
            Err(NameError::ControlCharacter)
        );
    }

    #[test]
    fn counts_characters_not_bytes() {
        let sixty_four = "é".repeat(MAX_NAME_CHARS);
        assert!(LibraryName::parse(&sixty_four).is_ok());
        assert_eq!(
            LibraryName::parse(&"é".repeat(MAX_NAME_CHARS + 1)),
            Err(NameError::TooLong)
        );
    }
}
