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

    /// Whether photographs are searched for what is in them.
    ///
    /// `people` includes it for the same reason every profile includes
    /// OCR: asking for more never takes away the cheaper thing.
    pub fn includes_objects(self) -> bool {
        matches!(self, Self::Photos | Self::People)
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
    /// Whether an object model and its runtime loaded at startup.
    pub objects: bool,
}

impl Capabilities {
    /// Asks the machine rather than assuming.
    ///
    /// Whether object recognition works is passed in rather than probed:
    /// the answer is whether the model loaded at startup, and loading it
    /// again on every request would cost a second each time.
    pub async fn detect(objects_loaded: bool) -> Self {
        Self {
            ocr: crate::ocr::is_available().await,
            objects: objects_loaded,
        }
    }

    /// The highest profile this machine can honour.
    ///
    /// Used to tell someone their choice will not do what they expect,
    /// not to silently overrule it: the setting is theirs, and hardware
    /// they add later should start working without them having to find
    /// this screen again.
    ///
    /// `photos` needs both halves of what it promises. Face grouping is
    /// not built, so `people` is never reported as supported.
    pub fn supported(self) -> Profile {
        match (self.ocr, self.objects) {
            (true, true) => Profile::Photos,
            (true, false) => Profile::Text,
            (false, _) => Profile::Off,
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
    fn only_photo_profiles_look_for_objects() {
        assert!(!Profile::Off.includes_objects());
        assert!(!Profile::Text.includes_objects());
        assert!(Profile::Photos.includes_objects());
        assert!(
            Profile::People.includes_objects(),
            "asking for faces did not ask to lose objects"
        );
    }

    #[test]
    fn a_machine_with_nothing_installed_supports_nothing() {
        assert_eq!(Capabilities::default().supported(), Profile::Off);
    }

    #[test]
    fn photos_are_supported_only_when_both_halves_are_there() {
        let text_only = Capabilities {
            ocr: true,
            objects: false,
        };
        let both = Capabilities {
            ocr: true,
            objects: true,
        };
        let objects_only = Capabilities {
            ocr: false,
            objects: true,
        };

        assert_eq!(text_only.supported(), Profile::Text);
        assert_eq!(both.supported(), Profile::Photos);
        assert_eq!(objects_only.supported(), Profile::Off);
    }
}
