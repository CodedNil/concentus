use serde::{Deserialize, Serialize};
use serde_with::{DeserializeFromStr, SerializeDisplay};
use std::{collections::HashMap, fmt, marker::PhantomData, str::FromStr};
use uuid::Uuid;

/// A typed UUID. Ordering follows the bytes, so v7 IDs sort by creation time.
#[derive(Serialize, Deserialize)]
#[serde(transparent)]
pub struct Id<T>(
    /// The underlying UUID.
    Uuid,
    /// Marks which entity this ID belongs to; takes no space.
    PhantomData<fn() -> T>,
);

impl<T> Id<T> {
    pub const fn from_uuid(uuid: Uuid) -> Self {
        Self(uuid, PhantomData)
    }

    pub const fn from_u128(value: u128) -> Self {
        Self::from_uuid(Uuid::from_u128(value))
    }

    pub const fn uuid(self) -> Uuid {
        self.0
    }

    /// Mint a `UUIDv7` from a Unix millisecond timestamp and caller-supplied random bytes.
    pub const fn new_v7(unix_ms: u64, random: &[u8; 10]) -> Self {
        Self::from_uuid(uuid::Builder::from_unix_timestamp_millis(unix_ms, random).into_uuid())
    }
}

// Written by hand because derives would require `T` to implement the traits too.
impl<T> Clone for Id<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> Copy for Id<T> {}

impl<T> PartialEq for Id<T> {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

impl<T> Eq for Id<T> {}

impl<T> std::hash::Hash for Id<T> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.0.hash(state);
    }
}

impl<T> PartialOrd for Id<T> {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl<T> Ord for Id<T> {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.0.cmp(&other.0)
    }
}

impl<T> Default for Id<T> {
    /// The nil UUID.
    fn default() -> Self {
        Self::from_uuid(Uuid::nil())
    }
}

impl<T> fmt::Debug for Id<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Id({})", self.0)
    }
}

impl<T> fmt::Display for Id<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl<T> FromStr for Id<T> {
    type Err = uuid::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Uuid::try_parse(s).map(Self::from_uuid)
    }
}

/// Something with a strength from 0 to 100 (tag score, relevance).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Scored<I> {
    /// What is being scored.
    pub id: I,
    /// Strength as a whole percentage from 0 to 100.
    pub weight: u8,
}

/// A link to an entity with a strength.
pub type Weighted<T> = Scored<Id<T>>;

/// A related record that can be omitted, returned as an ID, or expanded.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum RecordRef<T> {
    #[default]
    None,
    Id(Id<T>),
    Data(Box<T>),
}

/// An identifier in another system. Values are opaque text, even when they contain only digits.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ExternalId {
    pub provider: ExternalIdProvider,
    pub value: String,
}

impl ExternalId {
    pub fn new(provider: ExternalIdProvider, value: impl Into<String>) -> Self {
        Self {
            provider,
            value: value.into(),
        }
    }
}

impl From<Isrc> for ExternalId {
    fn from(isrc: Isrc) -> Self {
        Self::new(ExternalIdProvider::Isrc, isrc.as_str())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ExternalIdProvider {
    Musicbrainz,
    Isrc,
    Barcode,
    Wikidata,
    Discogs,
    Genius,
    AppleMusic,
    Deezer,
    Tidal,
    Spotify,
    Amazon,
    YoutubeMusic,
    Isni,
    Ipi,
    Other(String),
}

/// External IDs grouped by provider. A provider can have multiple values for a record.
pub type ExternalIds = HashMap<ExternalIdProvider, Vec<String>>;

/// International Standard Recording Code, e.g. `USRC17607839`. Stored as 12 ASCII bytes.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, SerializeDisplay, DeserializeFromStr,
)]
pub struct Isrc(
    /// The 12 ASCII characters, uppercase.
    [u8; 12],
);

impl Isrc {
    pub fn as_str(&self) -> &str {
        std::str::from_utf8(&self.0).unwrap_or_default()
    }
}

impl FromStr for Isrc {
    type Err = ParseError;

    /// Accepts hyphenated or plain, any case.
    fn from_str(s: &str) -> Result<Self, ParseError> {
        let mut code = [0u8; 12];
        let mut n = 0;
        for byte in s.bytes().filter(|b| *b != b'-') {
            if n == 12 {
                return Err(ParseError("invalid ISRC"));
            }
            code[n] = byte.to_ascii_uppercase();
            n += 1;
        }
        let ok = n == 12
            && code[..2].iter().all(u8::is_ascii_uppercase)
            && code[2..5]
                .iter()
                .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit())
            && code[5..].iter().all(u8::is_ascii_digit);
        if ok {
            Ok(Self(code))
        } else {
            Err(ParseError("invalid ISRC"))
        }
    }
}

impl fmt::Display for Isrc {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseError(
    /// What was wrong.
    pub(super) &'static str,
);

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.0)
    }
}

impl std::error::Error for ParseError {}
