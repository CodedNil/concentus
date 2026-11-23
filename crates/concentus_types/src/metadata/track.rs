use std::collections::HashMap;

use super::{
    Album, Artist, AudioFeatures, Credit, ExternalId, Id, Language, Lyrics, Tag, Visuals, Weighted,
    primary_artists,
};
use serde::{Deserialize, Serialize};
use url::Url;

/// A recording; `album_id` names its primary album, whose discs give the track's position.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Track {
    /// Unique ID, the same however many albums the recording is on.
    pub id: Id<Self>,
    /// Title without the version suffix.
    pub title: String,
    /// Length of the audio.
    pub duration_ms: u32,
    /// Version suffix, e.g. `"Live at Wembley"` or `"Remastered 2011"`.
    pub version: Option<String>,
    /// Everyone involved in billing order: primary and featured artists first, then other roles.
    pub credits: Vec<Credit>,
    /// The album it primarily appears on.
    pub album_id: Option<Id<Album>>,
    /// BCP 47 languages of the vocals; more than one for bilingual tracks.
    pub languages: Vec<Language>,
    /// Lyrics by language. API responses populate this only when requested.
    pub lyrics: HashMap<Language, Lyrics>,
    /// Whether the track is explicit or a clean edit; `None` if unrated.
    pub rating: Option<ContentRating>,
    /// Genres, moods and other tags with strengths.
    pub tags: Vec<Weighted<Tag>>,
    /// Tempo, key, loudness, danceability, valence, crossfade points and so on.
    pub features: AudioFeatures,
    /// The story behind the song.
    pub about: Option<String>,
    /// The tracks this one samples, covers or otherwise derives from.
    pub derived_from: Vec<Derivation>,
    /// Link to the music video on a video site such as `YouTube`.
    pub music_video: Option<Url>,
    /// Track artwork, canvas and colors.
    pub visuals: Visuals,
    /// The same recording in other systems, including its ISRCs.
    pub external_ids: Vec<ExternalId>,
}

impl Track {
    /// The billed main artists, in billing order.
    pub fn primary_artists(&self) -> impl Iterator<Item = Id<Artist>> + '_ {
        primary_artists(&self.credits)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ContentRating {
    /// Contains explicit content.
    Explicit,
    /// Some but not all of the content is explicit.
    PartiallyExplicit,
    /// A censored edit of explicit material.
    Clean,
}

/// A track another track is derived from, and how.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Derivation {
    /// The original track.
    pub track_id: Id<Track>,
    /// How this track uses it.
    pub kind: DerivationKind,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DerivationKind {
    /// Reuses a piece of the original recording.
    Sample {
        /// Where in this track the original appears.
        from_range: Option<TimeRange>,
        /// Which part of the original is used.
        to_range: Option<TimeRange>,
    },
    /// Re-performs a melody or lyric from the original.
    Interpolation,
    /// A new recording of the same composition by someone else.
    Cover,
    /// A reworked version of the original.
    Remix,
    /// Another version of the same recording: live, acoustic, remaster.
    Version,
}

/// A span of time inside a track.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct TimeRange {
    /// Start, from the beginning of the track.
    pub start_ms: u32,
    /// End, from the beginning of the track.
    pub end_ms: u32,
}
