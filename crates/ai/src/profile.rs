//! What the owner turned on, and what the machine can actually do.
//!
//! These are two different questions and the interface must not conflate
//! them. A person can ask for photo understanding on a machine that
//! cannot do it; the honest response is to say so, not to accept the
//! setting and quietly do nothing.

use std::fmt;

/// What the library owner has enabled, in increasing cost.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Profile {
    /// Nothing runs. The default, and what a library with no row has.
    #[default]
    Off,
    /// Reading text out of pictures of text, and later document
    /// embeddings. Cheap enough for a NAS or a small board.
    Text,
    /// Adds image understanding. Wants a real processor.
    Photos,
    /// Adds face grouping, which is opted into on its own rather than
    /// arriving with an upgrade.
    People,
}

impl Profile {
    pub fn parse(raw: &str) -> Option<Self> {
        match raw {
            "off" => Some(Self::Off),
            "text" => Some(Self::Text),
            "photos" => Some(Self::Photos),
            "people" => Some(Self::People),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::Text => "text",
            Self::Photos => "photos",
            Self::People => "people",
        }
    }

    /// Whether text recognition runs at this setting.
    ///
    /// Every profile above `off` includes it: a person who asked for
    /// photo understanding did not ask to lose the cheaper thing.
    pub fn includes_ocr(self) -> bool {
        !matches!(self, Self::Off)
    }

    pub fn is_off(self) -> bool {
        matches!(self, Self::Off)
    }
}

impl fmt::Display for Profile {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// What this deployment can actually do, as opposed to what was asked
/// for. Reported to the interface so it can say "this server has no text
/// recognition installed" instead of offering a switch that does
/// nothing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Capabilities {
    pub ocr: bool,
}

impl Capabilities {
    /// Asks the machine rather than assuming.
    pub async fn detect() -> Self {
        Self {
            ocr: crate::ocr::is_available().await,
        }
    }

    /// The highest profile this machine can honour.
    ///
    /// Used to tell someone their choice will not do what they expect,
    /// not to silently overrule it: the setting is theirs, and hardware
    /// they add later should start working without them having to find
    /// this screen again.
    pub fn supported(self) -> Profile {
        if self.ocr {
            Profile::Text
        } else {
            Profile::Off
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_profile_survives_a_round_trip() {
        for profile in [
            Profile::Off,
            Profile::Text,
            Profile::Photos,
            Profile::People,
        ] {
            assert_eq!(Profile::parse(profile.as_str()), Some(profile));
        }
    }

    #[test]
    fn an_unknown_profile_is_not_guessed_at() {
        assert_eq!(Profile::parse("magic"), None);
        assert_eq!(Profile::parse(""), None);
        assert_eq!(Profile::parse("OFF"), None);
    }

    #[test]
    fn the_default_is_off() {
        assert_eq!(Profile::default(), Profile::Off);
        assert!(Profile::default().is_off());
        assert!(!Profile::default().includes_ocr());
    }

    #[test]
    fn asking_for_more_never_takes_away_the_cheaper_thing() {
        // Someone who turns on photo understanding has not asked to stop
        // reading text out of their scans.
        for profile in [Profile::Text, Profile::Photos, Profile::People] {
            assert!(profile.includes_ocr(), "{profile} dropped text recognition");
        }
    }

    #[test]
    fn a_machine_with_nothing_installed_supports_nothing() {
        assert_eq!(Capabilities::default().supported(), Profile::Off);
        assert_eq!(Capabilities { ocr: true }.supported(), Profile::Text);
    }
}
