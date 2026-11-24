use super::{
    Artist, Artwork, Credit, ExternalIds, Id, PartialDate, RecordRef, Scheme, Scored, Tag, Track,
    primary_artists,
};
use serde::{Deserialize, Serialize};

/// A release; editions (deluxe, remaster, regional) are separate albums linked through `edition`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Album {
    /// Unique ID of this release.
    pub id: Id<Self>,
    /// The same album in other systems.
    pub external_ids: ExternalIds,

    /// Title without the edition suffix.
    pub title: String,
    /// Title for alphabetical ordering.
    pub sort_title: Option<String>,
    /// Set when this is an edition of another album; `None` for a base album.
    pub edition: Option<Edition>,
    /// The type of release, if known.
    pub album_type: Option<AlbumType>,
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
    /// Genres, moods and other weighted tags. Empty when omitted.
    pub tags: Vec<Scored<RecordRef<Tag>>>,

    /// Preferred square artwork.
    pub artwork_square: Option<Box<Artwork>>,
    /// Preferred banner artwork.
    pub artwork_banner: Option<Box<Artwork>>,
    /// Preferred motion square artwork.
    pub artwork_motion_square: Option<Box<Artwork>>,
    /// Page colors for the release.
    pub theme: Option<Scheme>,
}

impl Album {
    /// The preferred square cover.
    pub fn cover(&self) -> Option<&Artwork> {
        self.artwork_square.as_deref()
    }

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
        self.discs.iter().map(|disc| disc.tracks.len()).sum()
    }

    /// Every track on the album, disc by disc.
    pub fn track_ids(&self) -> impl Iterator<Item = Id<Track>> + '_ {
        self.discs
            .iter()
            .flat_map(|disc| disc.tracks.iter().filter_map(track_id))
    }

    /// The `(disc, track)` position of a track, both starting at 1.
    pub fn position(&self, track: Id<Track>) -> Option<(u16, u16)> {
        self.discs.iter().enumerate().find_map(|(d, disc)| {
            let t = disc
                .tracks
                .iter()
                .position(|entry| track_id(entry) == Some(track))?;
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

/// The type of an album or other release.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AlbumType {
    /// A full-length album.
    Album,
    /// A release designated as a single.
    Single,
    /// A release designated as an extended play.
    Ep,
    /// Music from a film, show, game, or other work.
    Soundtrack {
        /// The source work's title, if known.
        source: Option<String>,
    },
    /// A release outside the primary categories above.
    Other,
}

/// One disc of a release. Single-disc releases have exactly one.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Disc {
    /// Disc subtitle, e.g. `"Bonus"`.
    pub title: Option<String>,
    /// Tracks in play order. Position in this list is the track number.
    pub tracks: Vec<RecordRef<Track>>,
}

fn track_id(track: &RecordRef<Track>) -> Option<Id<Track>> {
    match track {
        RecordRef::None => None,
        RecordRef::Id(id) => Some(*id),
        RecordRef::Data(track) => Some(track.id),
    }
}

/// Artwork roles for a album.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AlbumArtworkKind {
    Square,
    Banner,
    MotionSquare,
}
