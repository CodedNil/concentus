//! Shared Rust contract for the Postcard HTTP API.

use crate::{Album, Artist, ExternalId, Id, Isrc, Tag, Track};
use serde::{Deserialize, Serialize, de::DeserializeOwned};

pub use postcard::Error as CodecError;

/// Increment this whenever the Postcard representation of a request or reply changes.
pub const SCHEMA_REVISION: &str = "2";
pub const SCHEMA_HEADER: &str = "x-concentus-schema";
pub const CONTENT_TYPE: &str = "application/vnd.concentus.postcard";

pub fn encode<T: Serialize + ?Sized>(value: &T) -> Result<Vec<u8>, CodecError> {
    postcard::to_allocvec(value)
}

pub fn decode<T: DeserializeOwned>(bytes: &[u8]) -> Result<T, CodecError> {
    let (value, rest) = postcard::take_from_bytes(bytes)?;
    if rest.is_empty() {
        Ok(value)
    } else {
        Err(CodecError::DeserializeBadEncoding)
    }
}

pub trait Collection: Serialize + DeserializeOwned {
    const NAME: &'static str;
}

impl Collection for Artist {
    const NAME: &'static str = "artists";
}
impl Collection for Album {
    const NAME: &'static str = "albums";
}
impl Collection for Track {
    const NAME: &'static str = "tracks";
}
impl Collection for Tag {
    const NAME: &'static str = "tags";
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApiError {
    pub message: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PageQuery {
    pub after: Option<String>,
    pub limit: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(bound = "")]
pub struct Lookup<T> {
    pub id: Option<Id<T>>,
    pub artist_name: Option<String>,
    pub external_ids: Vec<ExternalId>,
    pub fields: Vec<FieldFilter>,
    pub options: LookupOptions,
}

/// Selects additional response data. These options do not affect matching.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct LookupOptions {
    /// Include a track's lyrics in the response.
    pub lyrics: bool,
}

pub type ArtistLookup = Lookup<Artist>;
pub type AlbumLookup = Lookup<Album>;
pub type TrackLookup = Lookup<Track>;
pub type TagLookup = Lookup<Tag>;

impl<T> Default for Lookup<T> {
    fn default() -> Self {
        Self {
            id: None,
            artist_name: None,
            external_ids: Vec::new(),
            fields: Vec::new(),
            options: LookupOptions::default(),
        }
    }
}

impl<T> Lookup<T> {
    pub fn by_id(id: Id<T>) -> Self {
        Self {
            id: Some(id),
            ..Self::default()
        }
    }

    pub fn by_external_id(id: ExternalId) -> Self {
        Self {
            external_ids: vec![id],
            ..Self::default()
        }
    }

    /// Add a filter on a top-level field. All filters are combined with AND.
    #[must_use]
    pub fn with_field(mut self, key: impl Into<String>, value: impl Into<FieldValue>) -> Self {
        self.fields.push(FieldFilter {
            path: vec![PathSegment::Field(key.into())],
            value: value.into(),
        });
        self
    }
}

impl Lookup<Track> {
    #[must_use]
    pub const fn with_lyrics(mut self) -> Self {
        self.options.lyrics = true;
        self
    }

    pub fn by_isrc(isrc: Isrc) -> Self {
        Self::by_external_id(ExternalId::Isrc(isrc))
    }

    pub fn by_title(title: impl Into<String>) -> Self {
        Self::default().with_field("title", title.into())
    }
}

impl Lookup<Artist> {
    pub fn by_name(name: impl Into<String>) -> Self {
        Self::default().with_field("name", name.into())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FieldFilter {
    pub path: Vec<PathSegment>,
    pub value: FieldValue,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum PathSegment {
    Field(String),
    /// Groups predicates that must match the same array element. This is a label,
    /// independent of the element's actual position in the stored array.
    Element(u8),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum FieldValue {
    Text(String),
    Unsigned(u64),
    Signed(i64),
    Float(f32),
    Bool(bool),
    Null,
    Array,
    Object,
}

impl From<String> for FieldValue {
    fn from(value: String) -> Self {
        Self::Text(value)
    }
}

impl From<&str> for FieldValue {
    fn from(value: &str) -> Self {
        Self::Text(value.into())
    }
}

impl From<u64> for FieldValue {
    fn from(value: u64) -> Self {
        Self::Unsigned(value)
    }
}

impl From<bool> for FieldValue {
    fn from(value: bool) -> Self {
        Self::Bool(value)
    }
}
