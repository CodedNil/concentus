use super::super::artwork::web_url;
use super::Page;
use super::metadata::{artwork, credits, duration, fact, identifiers, related_link, tags};
use crate::catalog::Kind;
use concentus_types::{DerivationKind, Track};
use std::collections::HashMap;
use topcoat::{
    context::Cx,
    view::{BoxView, ViewExt, view},
};

pub(super) fn render<'a>(cx: &'a Cx, page: &'a Page, track: &'a Track) -> BoxView<'a> {
    view! {
        <section class="panel">
            <div class="panel-heading"><h2>"Recording details"</h2></div>
            <dl class="facts">
                if track.duration_ms > 0 {
                    (fact(cx, "Duration", Some(&duration(track.duration_ms))))
                }
                (fact(cx, "Version", track.version.as_deref()))
                (fact(
                    cx,
                    "Release date",
                    track.release_date.map(|date| date.to_string()).as_deref(),
                ))
                (fact(
                    cx,
                    "Content rating",
                    track.rating.map(|rating| format!("{rating:?}")).as_deref(),
                ))
                if !track.languages.is_empty() {
                    (fact(cx, "Languages", Some(&track.languages.join(", "))))
                }
                if let Some(url) = track.music_video.as_deref().and_then(web_url) {
                    <div>
                        <dt>"Music video"</dt>
                        <dd><a href=(url)>"Watch video ↗"</a></dd>
                    </div>
                }
            </dl>
            if let Some(about) = &track.about {
                <p class="panel-copy prose">(about)</p>
            }
        </section>
        (page.edit_fields(
            cx,
            &[
                "duration_ms",
                "version",
                "release_date",
                "rating",
                "languages",
                "music_video",
                "about",
            ],
        ))
        (credits(cx, page, &track.credits))
        (page.edit_fields(cx, &["credits"]))
        if !track.lyrics.is_empty() {
            <section class="panel">
                <div class="panel-heading"><h2>"Lyrics"</h2></div>
                for (language, lyrics) in sorted(&track.lyrics) {
                    <details open=(true) class="lyric">
                        <summary>(language)</summary>
                        if lyrics.instrumental {
                            <p>"Instrumental"</p>
                        }
                        for channel in &lyrics.channels {
                            <p class="channel">
                                (format!("{:?}", channel.kind))
                                if let Some(id) = channel.artist_id {
                                    " · "
                                    (related_link(cx, page, Kind::Artist, &id.to_string()))
                                }
                            </p>
                            <pre>
                                (channel
                                    .spans
                                    .iter()
                                    .map(|span| span.text.as_str())
                                    .collect::<String>())
                            </pre>
                        }
                    </details>
                }
            </section>
        }
        (page.edit_fields(cx, &["lyrics"]))
        (audio(cx, &track.features))
        (page.edit_fields(
            cx,
            &[
                "tempo_bpm",
                "key",
                "mode",
                "time_signature",
                "loudness_lufs",
                "peak",
                "acousticness",
                "danceability",
                "energy",
                "instrumentalness",
                "liveness",
                "speechiness",
                "valence",
            ],
        ))
        if !track.derived_from.is_empty() {
            <section class="panel">
                <div class="panel-heading"><h2>"Derived from"</h2></div>
                <ul class="metadata-list">
                    for (id, kinds) in sorted(&track.derived_from) {
                        <li>
                            (related_link(cx, page, Kind::Track, &id.to_string()))
                            <small>
                                (kinds
                                    .iter()
                                    .map(derivation)
                                    .collect::<Vec<_>>()
                                    .join(" · "))
                            </small>
                        </li>
                    }
                </ul>
            </section>
        }
        (page.edit_fields(cx, &["derived_from"]))
        (tags(cx, &track.tags))
        (page.edit_fields(cx, &["tags"]))
        (artwork(cx, [("canvas", track.artwork_canvas.as_deref())]))
        (page.edit_fields(cx, &["artwork_canvas"]))
        (identifiers(cx, Kind::Track, &page.id, &track.external_ids))
        (page.edit_fields(cx, &["external_ids"]))
    }
    .boxed()
}

fn audio<'a>(cx: &'a Cx, features: &'a concentus_types::AudioFeatures) -> BoxView<'a> {
    let values = [
        (
            "Tempo",
            features.tempo_bpm.map(|value| format!("{value} BPM")),
        ),
        ("Key", features.key.map(|value| format!("{value:?}"))),
        ("Mode", features.mode.map(|value| format!("{value:?}"))),
        (
            "Beats per bar",
            features.time_signature.map(|value| value.to_string()),
        ),
        (
            "Loudness",
            features.loudness_lufs.map(|value| format!("{value} LUFS")),
        ),
        ("Peak", features.peak.map(|value| value.to_string())),
        (
            "Acousticness",
            features.acousticness.map(|value| format!("{value}%")),
        ),
        (
            "Danceability",
            features.danceability.map(|value| format!("{value}%")),
        ),
        ("Energy", features.energy.map(|value| format!("{value}%"))),
        (
            "Instrumentalness",
            features.instrumentalness.map(|value| format!("{value}%")),
        ),
        (
            "Liveness",
            features.liveness.map(|value| format!("{value}%")),
        ),
        (
            "Speechiness",
            features.speechiness.map(|value| format!("{value}%")),
        ),
        ("Valence", features.valence.map(|value| format!("{value}%"))),
    ];
    view! {
        if values.iter().any(|(_, value)| value.is_some()) {
            <section class="panel">
                <div class="panel-heading"><h2>"Audio analysis"</h2></div>
                <dl class="facts">
                    for (label, value) in &values {
                        (fact(cx, label, value.as_deref()))
                    }
                </dl>
            </section>
        }
    }
    .boxed()
}

fn derivation(kind: &DerivationKind) -> String {
    match kind {
        DerivationKind::Sample {
            from_range,
            to_range,
        } => {
            let span = |range: &concentus_types::TimeRange| {
                format!("{}–{} ms", range.start_ms, range.end_ms)
            };
            let mut parts = vec!["Sample".to_owned()];
            if let Some(range) = from_range {
                parts.push(format!("here {}", span(range)));
            }
            if let Some(range) = to_range {
                parts.push(format!("source {}", span(range)));
            }
            parts.join(" · ")
        }
        _ => format!("{kind:?}"),
    }
}

fn sorted<K: Ord, V>(entries: &HashMap<K, V>) -> Vec<(&K, &V)> {
    let mut entries: Vec<_> = entries.iter().collect();
    entries.sort_unstable_by_key(|(key, _)| *key);
    entries
}
