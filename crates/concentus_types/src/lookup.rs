//! Lookup requests and response options.

use crate::{Album, Artist, ExternalId, Id, Isrc, Tag, Track};
pub use filter::{FieldFilter, FieldValue, PathSegment};
use serde::{Deserialize, Serialize};

mod filter;

/// Match a single record using every supplied criterion.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(bound = "")]
pub struct Lookup<T> {
    /// The Concentus ID of the record.
    pub id: Option<Id<T>>,
    /// An artist's name or the name of a billed artist on an album or track.
    pub artist_name: Option<String>,
    /// Identifiers to match in other catalogs.
    pub external_ids: Vec<ExternalId>,
    /// Additional field predicates, all of which must match.
    pub fields: Vec<FieldFilter>,
    pub options: LookupOptions,
}

pub type ArtistLookup = Lookup<Artist>;
pub type AlbumLookup = Lookup<Album>;
pub type TrackLookup = Lookup<Track>;
pub type TagLookup = Lookup<Tag>;

// Default construction does not require a default record of type T.
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

    /// Select tag data for an artist, album, or track lookup.
    #[must_use]
    pub const fn with_tags(mut self, tags: RelatedData) -> Self {
        self.options.tags = tags;
        self
    }

    /// Select visual data for an artist, album, or track lookup.
    #[must_use]
    pub const fn with_visuals(mut self, visuals: VisualData) -> Self {
        self.options.visuals = visuals;
        self
    }
}

impl Lookup<Track> {
    #[must_use]
    pub const fn with_album(mut self, album: RelatedData) -> Self {
        self.options.album = album;
        self
    }

    #[must_use]
    pub const fn with_lyrics(mut self) -> Self {
        self.options.lyrics = true;
        self
    }

    pub fn by_isrc(isrc: Isrc) -> Self {
        Self::by_external_id(isrc.into())
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

impl Lookup<Album> {
    /// Select how much of the album's tracklist to include.
    #[must_use]
    pub const fn with_tracks(mut self, tracks: RelatedData) -> Self {
        self.options.tracks = tracks;
        self
    }
}

/// Selects additional response data. These options do not affect matching.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct LookupOptions {
    /// Include a track's lyrics in the response.
    pub lyrics: bool,
    /// How much of a track's primary album to include.
    pub album: RelatedData,
    /// How much of the matched artist's, album's, or track's tags to include.
    pub tags: RelatedData,
    /// How much artwork and styling to include.
    pub visuals: VisualData,
    /// How much of an album's tracklist to include.
    pub tracks: RelatedData,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum VisualData {
    /// Omit artwork and styling.
    None,
    /// Include the primary cover or portrait and its renditions.
    Primary,
    /// Include all artwork and styling.
    #[default]
    Full,
}

/// Amount of related data returned by a lookup.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum RelatedData {
    None,
    #[default]
    Ids,
    Full,
}
