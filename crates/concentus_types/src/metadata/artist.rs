use super::{ExternalIds, Font, Id, PartialDate, RecordRef, Role, Scored, Tag, Visuals};
use serde::{Deserialize, Serialize};

/// A person, group or other credited act, with its page decorated through `visuals`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Artist {
    /// Unique ID.
    pub id: Id<Self>,
    /// The same artist in other systems.
    pub external_ids: ExternalIds,

    /// Display name.
    pub name: String,
    /// Name for alphabetical ordering, e.g. `"Beatles, The"`.
    pub sort_name: Option<String>,
    /// Person or group; groups carry their members.
    pub kind: ArtistKind,

    /// Where the artist is from, e.g. `"London, England"`.
    pub origin: Option<String>,
    /// Born or formed.
    pub begin: Option<PartialDate>,
    /// Died or disbanded.
    pub end: Option<PartialDate>,

    /// Biography.
    pub biography: Option<String>,
    /// Genres, moods and other weighted tags. Empty when omitted.
    pub tags: Vec<Scored<RecordRef<Tag>>>,

    /// Portraits, banners, logos, motion header and colors.
    pub visuals: Visuals,
    /// Custom font for the artist's name and page headings.
    pub font: Option<Box<Font>>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ArtistKind {
    /// One individual.
    #[default]
    Person,
    /// A band, duo, orchestra, choir or other ensemble.
    Group {
        /// Current and past members.
        members: Vec<Membership>,
    },
    /// Not known.
    Unknown,
}

/// A member of a group.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Membership {
    /// The member.
    pub artist_id: Id<Artist>,
    /// What they did in the group, e.g. an instrument.
    pub roles: Vec<Role>,

    /// When the member joined the group, if known.
    pub joined: Option<PartialDate>,
    /// When the member left the group, if known.
    pub left: Option<PartialDate>,
}
