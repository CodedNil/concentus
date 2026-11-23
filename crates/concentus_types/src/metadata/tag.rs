use super::Id;
use serde::{Deserialize, Serialize};

/// A genre, mood or activity that music is tagged with.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Tag {
    /// Unique ID.
    pub id: Id<Self>,
    /// Display name.
    pub name: String,
    /// Whether this is a genre, an emotion or an activity.
    pub kind: TagKind,
    /// Other names for the same tag.
    pub aliases: Vec<String>,
    /// The broader tag this one belongs to; mostly used to arrange genres as a tree.
    pub parent: Option<Id<Self>>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum TagKind {
    /// A style of music: ambient, drill, bossa nova.
    #[default]
    Genre,
    /// How it feels: melancholy, euphoric.
    Emotion,
    /// What it's for or where it fits: workout, focus, road trip.
    Activity,
}
