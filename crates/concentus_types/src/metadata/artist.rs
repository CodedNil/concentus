use super::{ExternalId, Id, PartialDate, Role, Tag, Visuals, Weighted};
use serde::{Deserialize, Serialize};

/// A person, group or other credited act, with its page decorated through `visuals`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Artist {
    /// Unique ID.
    pub id: Id<Self>,
    /// Display name.
    pub name: String,
    /// Name for alphabetical ordering, e.g. `"Beatles, The"`.
    pub sort_name: Option<String>,
    /// Person or group; groups carry their members.
    pub kind: ArtistKind,
    /// Biography.
    pub biography: Option<String>,
    /// Born or formed.
    pub begin: Option<PartialDate>,
    /// Died or disbanded.
    pub end: Option<PartialDate>,
    /// Where the artist is from, e.g. `"London, England"`.
    pub origin: Option<String>,
    /// Genres, moods and other tags with strengths.
    pub tags: Vec<Weighted<Tag>>,
    /// Portraits, banners, logos, motion header, colors and fonts.
    pub visuals: Visuals,
    /// The same artist in other systems.
    pub external_ids: Vec<ExternalId>,
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
}
