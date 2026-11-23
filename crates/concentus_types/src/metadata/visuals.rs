use super::ParseError;
use serde::{Deserialize, Serialize};
use serde_with::{DeserializeFromStr, SerializeDisplay};
use std::{fmt, str::FromStr};

/// Everything visual about an entity.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Visuals {
    /// Covers, portraits, banners, logos and animated loops.
    pub artwork: Vec<Artwork>,
    /// Colors for the entity's page.
    pub theme: Theme,
}

impl Visuals {
    /// The front cover, preferring one that isn't explicit.
    pub fn cover(&self) -> Option<&Artwork> {
        let mut covers = self.artwork.iter().filter(|a| a.kind == ArtworkKind::Cover);
        covers
            .clone()
            .find(|a| !a.explicit)
            .or_else(|| covers.next())
    }
}

/// One piece of artwork in every size and format available, still or animated.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Artwork {
    /// What the artwork shows.
    pub kind: ArtworkKind,
    /// True if the artwork itself is explicit; a clean alternate may exist.
    pub explicit: bool,

    /// The same artwork at different sizes; video renditions make it animated.
    pub files: Vec<Rendition>,

    /// Prominent colors for theming, most prominent first.
    pub colors: Vec<Rgb>,
    /// Compact placeholder to show while loading.
    pub blurhash: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ArtworkKind {
    /// Front cover.
    Cover,
    /// Back cover.
    Back,
    /// Portrait of an artist.
    Portrait,
    /// Wide header image.
    Banner,
    /// Logo or wordmark.
    Logo,
    /// Short vertical loop shown behind a playing track.
    Canvas,
}

/// One file of an artwork: an image, or a video loop when it has a duration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rendition {
    /// Where the file is served from.
    pub url: String,

    /// Width in pixels.
    pub width: u16,
    /// Height in pixels.
    pub height: u16,

    /// Codec string for video, e.g. `hvc1`, `avc1`, `av01`.
    pub codec: Option<String>,
    /// Length of one loop; only set for video.
    pub duration_ms: Option<u32>,
}

/// Page colors for light and dark appearances.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Theme {
    /// Colors for light mode.
    pub light: Option<Scheme>,
    /// Colors for dark mode.
    pub dark: Option<Scheme>,
}

/// A complete set of UI colors for one appearance (light or dark).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Scheme {
    /// Page background.
    pub background: Rgb,
    /// Main text color.
    pub foreground: Rgb,
    /// Highlight color for buttons and active controls.
    pub accent: Rgb,
}

/// An sRGB color, written `#rrggbb`.
#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, Hash, SerializeDisplay, DeserializeFromStr,
)]
pub struct Rgb(
    /// Red.
    pub u8,
    /// Green.
    pub u8,
    /// Blue.
    pub u8,
);

impl fmt::Display for Rgb {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "#{:02x}{:02x}{:02x}", self.0, self.1, self.2)
    }
}

impl FromStr for Rgb {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, ParseError> {
        const BAD: ParseError = ParseError("invalid color, expected #rrggbb");
        let hex = s.strip_prefix('#').unwrap_or(s);
        if hex.len() != 6 || !hex.is_ascii() {
            return Err(BAD);
        }
        let channel = |i: usize| u8::from_str_radix(&hex[i..i + 2], 16).map_err(|_| BAD);
        Ok(Self(channel(0)?, channel(2)?, channel(4)?))
    }
}

/// A web font file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Font {
    /// Font family name, as used in CSS.
    pub family: String,
    /// Where the woff2 file is served from.
    pub url: String,
}
