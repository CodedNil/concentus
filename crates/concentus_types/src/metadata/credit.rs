use super::{Artist, Id};
use serde::{Deserialize, Serialize};

/// One person or group's contribution; people are [`Artist`]s of kind `Person`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Credit {
    /// Who contributed.
    pub artist_id: Id<Artist>,
    /// What they did.
    pub role: Role,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Role {
    /// Billed main artist.
    Primary,
    /// General performer.
    Performer,
    /// Lead vocals.
    Vocals,
    /// Backing vocals.
    BackgroundVocals,
    /// Plays an instrument, named, e.g. `"upright bass"`.
    Instrument(String),
    /// Guest artist billed alongside the main artist.
    Featured,
    /// Wrote the music.
    Composer,
    /// Wrote the words.
    Lyricist,
    /// Wrote music or words, unspecified.
    Writer,
    /// Adapted the composition for these performers.
    Arranger,
    /// Produced the recording.
    Producer,
    /// Financed and oversaw production.
    ExecutiveProducer,
    /// Recorded the audio.
    Engineer,
    /// Mixed the recording.
    Mixer,
    /// Mastered the recording.
    Mastering,
    /// Remixed the original.
    Remixer,
    /// Any other role, in the label's words.
    Other(String),
}

/// The billed main artists, in billing order.
pub fn primary_artists(credits: &[Credit]) -> impl Iterator<Item = Id<Artist>> + '_ {
    credits
        .iter()
        .filter(|c| c.role == Role::Primary)
        .map(|c| c.artist_id)
}
