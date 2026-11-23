use super::{
    Artist, Credit, ExternalId, Id, PartialDate, Tag, Track, Visuals, Weighted, primary_artists,
};
use serde::{Deserialize, Serialize};

/// A release; editions (deluxe, remaster, regional) are separate albums linked through `edition`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Album {
    /// Unique ID of this release.
    pub id: Id<Self>,
    /// Title without the edition suffix.
    pub title: String,
    /// Title for alphabetical ordering.
    pub sort_title: Option<String>,
    /// Set when this is an edition of another album; `None` for a base album.
    pub edition: Option<Edition>,
    /// Album, single, EP, compilation and so on.
    pub kind: AlbumKind,
    /// Release date, as precisely as it's known.
    pub release_date: Option<PartialDate>,
    /// Discs in order; the disc number is the position in this list.
    pub discs: Vec<Disc>,
    /// Album-level credits in billing order: the primary artists, then any executive producers.
    pub credits: Vec<Credit>,
    /// The label that released it.
    pub label: Option<String>,
    /// Copyright line as delivered, e.g. `"℗ 2026 Example Records"`.
    pub copyright: Option<String>,
    /// Editorial description.
    pub description: Option<String>,
    /// Genres, moods and other tags with strengths.
    pub tags: Vec<Weighted<Tag>>,
    /// Cover art, motion art and colors.
    pub visuals: Visuals,
    /// The same album in other systems.
    pub external_ids: Vec<ExternalId>,
}

impl Album {
    /// The billed main artists, in billing order.
    pub fn primary_artists(&self) -> impl Iterator<Item = Id<Artist>> + '_ {
        primary_artists(&self.credits)
    }

    /// Title with the edition in brackets, e.g. `"Blue Hour (Deluxe)"`.
    pub fn display_title(&self) -> String {
        self.edition.as_ref().map_or_else(
            || self.title.clone(),
            |edition| format!("{} ({})", self.title, edition.name),
        )
    }

    /// Number of tracks across all discs.
    pub fn track_count(&self) -> usize {
        self.discs.iter().map(|d| d.track_ids.len()).sum()
    }

    /// Every track on the album, disc by disc.
    pub fn track_ids(&self) -> impl Iterator<Item = Id<Track>> + '_ {
        self.discs.iter().flat_map(|d| d.track_ids.iter().copied())
    }

    /// The `(disc, track)` position of a track, both starting at 1.
    pub fn position(&self, track: Id<Track>) -> Option<(u16, u16)> {
        self.discs.iter().enumerate().find_map(|(d, disc)| {
            let t = disc.track_ids.iter().position(|id| *id == track)?;
            Some((u16::try_from(d + 1).ok()?, u16::try_from(t + 1).ok()?))
        })
    }
}

/// A named variant of another album, e.g. a deluxe or remastered release.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Edition {
    /// Edition name, e.g. `"Deluxe"` or `"2011 Remaster"`.
    pub name: String,
    /// The base album, once matched; the original release date is that album's.
    pub of: Option<Id<Album>>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum AlbumKind {
    /// A full-length album.
    #[default]
    Album,
    /// One or two songs released alone.
    Single,
    /// An extended play, longer than a single and shorter than an album.
    Ep,
    /// A collection of previously released songs, often by several artists.
    Compilation,
    /// Music from a film, show or game.
    Soundtrack {
        /// Title of the film, show or game it is from.
        source: Option<String>,
    },
    /// Recorded at a live performance.
    Live,
    /// Remixes of existing songs.
    Remix,
    /// Anything else.
    Other,
}

/// One disc of a release. Single-disc releases have exactly one.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Disc {
    /// Disc subtitle, e.g. `"Bonus"`.
    pub title: Option<String>,
    /// Tracks in play order. Position in this list is the track number.
    pub track_ids: Vec<Id<Track>>,
}
