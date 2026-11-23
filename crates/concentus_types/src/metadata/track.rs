use super::{
    Album, Artist, Artwork, ArtworkKind, AudioFeatures, Credit, ExternalIds, Id, Language, Lyrics,
    PartialDate, RecordRef, Scored, Tag, primary_artists,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A recording; its primary album's discs give the track's position.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Track {
    /// Unique ID, the same however many albums the recording is on.
    pub id: Id<Self>,
    /// The same recording in other systems, including its ISRCs.
    pub external_ids: ExternalIds,

    /// Title without the version suffix.
    pub title: String,
    /// Version suffix, e.g. `"Live at Wembley"` or `"Remastered 2011"`.
    pub version: Option<String>,
    /// Length of the audio.
    pub duration_ms: u32,
    /// Release date of this track, falling back to its primary album's date in API responses.
    pub release_date: Option<PartialDate>,
    /// Whether the track is explicit or a clean edit; `None` if unrated.
    pub rating: Option<ContentRating>,

    /// Everyone involved in billing order: primary and featured artists first, then other roles.
    pub credits: Vec<Credit>,
    /// The primary album, when known and requested.
    pub album: RecordRef<Album>,
    /// How this track derives from each source track, in order per source.
    pub derived_from: HashMap<Id<Self>, Vec<DerivationKind>>,

    /// BCP 47 languages of the vocals; more than one for bilingual tracks.
    pub languages: Vec<Language>,
    /// Lyrics by language. API responses populate this only when requested.
    pub lyrics: HashMap<Language, Lyrics>,

    /// Genres, moods and other weighted tags. Empty when omitted.
    pub tags: Vec<Scored<RecordRef<Tag>>>,
    /// Tempo, key, loudness, danceability, valence, crossfade points and so on.
    pub features: AudioFeatures,
    /// The story behind the song.
    pub about: Option<String>,

    /// Track artwork and canvas loops.
    pub artwork: Vec<Artwork>,
    /// Link to the music video on a video site such as `YouTube`.
    pub music_video: Option<String>,
}

impl Track {
    /// The track's own cover, preferring one that isn't explicit.
    pub fn cover(&self) -> Option<&Artwork> {
        Artwork::preferred(&self.artwork, ArtworkKind::Cover)
    }

    /// The primary album ID, whether the response includes the full album or just the ID.
    pub fn album_id(&self) -> Option<Id<Album>> {
        match &self.album {
            RecordRef::None => None,
            RecordRef::Id(id) => Some(*id),
            RecordRef::Data(album) => Some(album.id),
        }
    }

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
