use super::Id;
use serde::{Deserialize, Serialize};

/// A genre, mood or activity that music is tagged with.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Tag {
    /// Unique ID.
    pub id: Id<Self>,

    /// Display name.
    pub name: String,
    /// Classification used when browsing and organizing tags.
    pub kind: TagKind,
    /// A broader tag or the canonical tag this name refers to.
    pub relation: TagRelation,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum TagKind {
    /// A style of music: ambient, drill, bossa nova.
    Genre,
    /// How it feels: melancholy, euphoric.
    Mood,
    /// What it's for or where it fits: workout, focus, road trip.
    Activity,
    /// Imported or awaiting classification.
    #[default]
    Unclassified,
    /// Omitted from normal catalog views.
    Hidden,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum TagRelation {
    #[default]
    None,
    Parent(Id<Tag>),
    AliasOf(Id<Tag>),
}
