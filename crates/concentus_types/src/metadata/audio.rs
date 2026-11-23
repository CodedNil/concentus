use serde::{Deserialize, Serialize};

/// Analysis of the recording. Scores are whole percentages from 0 to 100.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct AudioFeatures {
    /// Tempo in beats per minute.
    pub tempo_bpm: Option<f32>,
    /// Tonic of the key.
    pub key: Option<PitchClass>,
    /// Major or minor.
    pub mode: Option<Mode>,
    /// Beats per bar, e.g. 4 for 4/4.
    pub time_signature: Option<u8>,
    /// Integrated loudness; players derive replay gain from it.
    pub loudness_lufs: Option<f32>,
    /// Linear sample peak, `0.0..=1.0` (above 1 if clipped).
    pub peak: Option<f32>,
    /// Confidence the track is acoustic rather than electronic.
    pub acousticness: Option<u8>,
    /// How suitable it is for dancing: tempo, rhythm stability, beat strength.
    pub danceability: Option<u8>,
    /// Perceived intensity and activity.
    pub energy: Option<u8>,
    /// Likelihood of no vocals.
    pub instrumentalness: Option<u8>,
    /// Likelihood it was recorded with a live audience.
    pub liveness: Option<u8>,
    /// Presence of spoken words.
    pub speechiness: Option<u8>,
    /// Musical positiveness: low is sad or angry, high is happy or euphoric.
    pub valence: Option<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PitchClass {
    /// C
    C,
    /// C sharp / D flat
    CSharp,
    /// D
    D,
    /// D sharp / E flat
    DSharp,
    /// E
    E,
    /// F
    F,
    /// F sharp / G flat
    FSharp,
    /// G
    G,
    /// G sharp / A flat
    GSharp,
    /// A
    A,
    /// A sharp / B flat
    ASharp,
    /// B
    B,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Mode {
    /// Major scale.
    Major,
    /// Minor scale.
    Minor,
}
