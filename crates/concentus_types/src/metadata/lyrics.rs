use super::{Artist, Id};
use serde::{Deserialize, Serialize};

/// A BCP 47 language tag such as `en`, `pt-BR`, or `und` for an unknown language.
pub type Language = String;

/// The words of a track in one language, sung on one or more channels.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Lyrics {
    /// True for tracks with no vocals; `channels` is then empty.
    pub instrumental: bool,
    /// One channel per voice; they run in parallel and may overlap in time.
    pub channels: Vec<Channel>,
}

/// The lines of one voice or sound source; an artist can have several, e.g. a lead and its backing vocals.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Channel {
    /// The artist who sings or makes it, if known.
    pub artist_id: Option<Id<Artist>>,
    /// What this channel carries.
    pub kind: ChannelKind,
    /// The spans in order; a span ending in a newline ends a line, so joining their text gives the lyrics.
    pub spans: Vec<Span>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChannelKind {
    /// The main vocal.
    Lead,
    /// Backing vocals sung behind the lead.
    Backing,
    /// A harmony line sung in parallel with the lead.
    Harmony,
    /// Spontaneous ad-libs and interjections.
    AdLib,
    /// Spoken rather than sung, e.g. a monologue or an interlude.
    Spoken,
    /// A group or audience singing along.
    Crowd,
    /// Descriptions of non-vocal sounds for captions, e.g. a guitar solo or applause.
    SoundCue,
}

/// The smallest timed unit of the lyrics, a word or part of a word; `text` keeps its trailing space or newline so spans rebuild the lyrics.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Span {
    /// The sung text, including any trailing space or newline; a word split in two has no space between its spans.
    pub text: String,
    /// When this span starts, from the start of the track; `None` for untimed lyrics.
    pub start_ms: Option<u32>,
    /// When this span ends; if missing, players use the next span's start.
    pub end_ms: Option<u32>,
}
