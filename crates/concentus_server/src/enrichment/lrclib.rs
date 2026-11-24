//! Keyless lyrics source: <https://lrclib.net/docs>.

use super::http::Source;
use crate::store::Store;
use concentus_types::Span;
use serde::Deserialize;
use sqlx::{query, query_as, query_scalar};
use topcoat::Result;

#[derive(sqlx::FromRow)]
struct Candidate {
    title: String,
    artist: String,
    album: Option<String>,
    duration_ms: u32,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Response {
    id: u64,
    duration: f64,
    instrumental: bool,
    plain_lyrics: Option<String>,
    synced_lyrics: Option<String>,
}

pub async fn enrich(
    source: &Source,
    store: &Store,
    id: &str,
    refresh: bool,
    generation: i64,
) -> Result<bool> {
    let candidate: Option<Candidate> = query_as(
        "SELECT track.title, track.duration_ms, album.title AS album, (
            SELECT artist.name FROM track_credits AS credit JOIN artists AS artist ON artist.id = credit.artist_id
            WHERE credit.owner_id = track.id AND credit.role = 'Primary' ORDER BY credit.position LIMIT 1
        ) AS artist FROM tracks AS track LEFT JOIN albums AS album ON album.id = track.album_id
        WHERE track.id = ? AND track.duration_ms BETWEEN 1000 AND 3600000
        AND (? OR NOT EXISTS (SELECT 1 FROM track_lyrics WHERE track_id = track.id))
        AND EXISTS (SELECT 1 FROM track_credits WHERE owner_id = track.id AND role = 'Primary')",
    ).bind(id).bind(refresh).fetch_optional(&store.0).await?;
    let Some(candidate) = candidate else {
        return Ok(false);
    };
    let duration = f64::from(candidate.duration_ms) / 1000.0;
    let seconds = duration.to_string();
    let mut params = vec![
        ("track_name", candidate.title.as_str()),
        ("artist_name", candidate.artist.as_str()),
        ("duration", &seconds),
    ];
    if let Some(album) = &candidate.album {
        params.push(("album_name", album));
    }
    let Some(response): Option<Response> = source.get(&params).await? else {
        return Ok(false);
    };
    if (response.duration - duration).abs() > 2.0 {
        return Ok(false);
    }
    let mut spans = response
        .synced_lyrics
        .as_deref()
        .map_or_else(Vec::new, parse_lrc);
    if !response.instrumental && !spans.iter().any(|span| !span.text.trim().is_empty()) {
        let Some(text) = response.plain_lyrics.filter(|text| !text.trim().is_empty()) else {
            return Ok(false);
        };
        spans = vec![Span {
            text,
            start_ms: None,
            end_ms: None,
        }];
    }
    let mut db = store.0.begin_with("BEGIN IMMEDIATE").await?;
    let active: bool = query_scalar(
        "SELECT EXISTS(SELECT 1 FROM enrichment_needed
        WHERE collection = 'tracks' AND record_id = ? AND field = 'lyrics' AND generation = ?)",
    )
    .bind(id)
    .bind(generation)
    .fetch_one(&mut *db)
    .await?;
    if !active {
        return Ok(false);
    }
    let before = crate::editor::snapshot(&mut db, crate::catalog::Kind::Track, id).await?;
    if refresh {
        query("DELETE FROM track_lyrics WHERE track_id = ? AND language = 'und'")
            .bind(id)
            .execute(&mut *db)
            .await?;
    }
    let inserted = query(
        "INSERT INTO track_lyrics (track_id, language, instrumental)
        SELECT ?, 'und', ? WHERE ? OR NOT EXISTS (SELECT 1 FROM track_lyrics WHERE track_id = ?)",
    )
    .bind(id)
    .bind(response.instrumental)
    .bind(refresh)
    .bind(id)
    .execute(&mut *db)
    .await?;
    if inserted.rows_affected() == 0 {
        return Ok(false);
    }
    if !response.instrumental {
        let channel: i64 = query_scalar(
            "INSERT INTO lyrics_channels (track_id, language, position, kind)
            VALUES (?, 'und', 0, 'Lead') RETURNING id",
        )
        .bind(id)
        .fetch_one(&mut *db)
        .await?;
        for (position, span) in spans.iter().enumerate() {
            query("INSERT INTO channel_spans (owner_id, position, text, start_ms, end_ms) VALUES (?, ?, ?, ?, ?)")
                .bind(channel).bind(i64::try_from(position)?).bind(&span.text).bind(span.start_ms).bind(span.end_ms)
                .execute(&mut *db).await?;
        }
    }
    query("INSERT INTO track_external_ids (owner_id, provider, value) VALUES (?, 'other:LRCLIB', ?) ON CONFLICT DO NOTHING")
        .bind(id).bind(response.id.to_string()).execute(&mut *db).await?;
    crate::editor::audit(&mut db, crate::catalog::Kind::Track, id, &before).await?;
    db.commit().await?;
    Ok(true)
}

fn parse_lrc(input: &str) -> Vec<Span> {
    let offset = input
        .lines()
        .find_map(|line| {
            line.trim()
                .strip_prefix("[offset:")?
                .strip_suffix(']')?
                .trim()
                .parse::<i64>()
                .ok()
        })
        .unwrap_or_default();
    let mut lines = Vec::new();
    for mut line in input.lines() {
        let mut starts = Vec::new();
        while let Some((label, rest)) = line
            .strip_prefix('[')
            .and_then(|value| value.split_once(']'))
        {
            let Some(time) = timestamp(label) else {
                break;
            };
            starts.push(
                u32::try_from((i64::from(time).saturating_add(offset)).max(0)).unwrap_or(u32::MAX),
            );
            line = rest;
        }
        for start in starts {
            lines.push((start, line.strip_prefix(' ').unwrap_or(line)));
        }
    }
    lines.sort_by_key(|&(start, _)| start);
    let mut spans = Vec::with_capacity(lines.len());
    let mut previous = None;
    let mut end_ms = None;
    for &(start, text) in lines.iter().rev() {
        if previous != Some(start) {
            end_ms = previous;
            previous = Some(start);
        }
        spans.push(Span {
            text: format!("{text}\n"),
            start_ms: Some(start),
            end_ms,
        });
    }
    spans.reverse();
    spans
}

fn timestamp(label: &str) -> Option<u32> {
    let (minutes, seconds) = label.split_once(':')?;
    let (seconds, fraction) = seconds.split_once('.').unwrap_or((seconds, ""));
    let seconds: u32 = seconds.parse().ok()?;
    if seconds >= 60 || !fraction.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    let millis = fraction
        .bytes()
        .take(3)
        .zip([100, 10, 1])
        .map(|(digit, factor)| u32::from(digit - b'0') * factor)
        .sum::<u32>();
    minutes
        .parse::<u32>()
        .ok()?
        .checked_mul(60_000)?
        .checked_add(seconds * 1000 + millis)
}
