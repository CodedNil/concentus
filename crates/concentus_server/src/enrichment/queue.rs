use super::{Enrichment, import, lrclib, musicbrainz::Batch, wikidata, wikimedia_commons};
use crate::{catalog::Kind, editor, store::Store};
use serde_json::{Value, json};
use sqlx::{AssertSqlSafe, query, query_as, query_scalar};
use std::{collections::BTreeMap, time::Duration};
use tokio::time::{MissedTickBehavior, interval};
use topcoat::Result;

#[derive(sqlx::FromRow)]
struct Job {
    collection: String,
    record_id: String,
    field: String,
    generation: i64,
    refresh: bool,
}

impl Enrichment {
    pub fn worker(self, store: Store) {
        tokio::spawn(async move {
            let mut ticker = interval(Duration::from_secs(60));
            ticker.set_missed_tick_behavior(MissedTickBehavior::Skip);
            // Let the server start accepting requests before the first pass.
            ticker.tick().await;
            loop {
                tokio::select! { _ = ticker.tick() => {}, () = self.wake.notified() => {} }
                if let Err(error) = self.work(&store).await {
                    eprintln!("Enrichment queue failed: {error}");
                }
            }
        });
    }

    async fn work(&self, store: &Store) -> Result<()> {
        let jobs: Vec<Job> = query_as(
            "SELECT collection, record_id, field, generation, refresh FROM enrichment_needed
            WHERE due_at <= unixepoch() ORDER BY refresh DESC, due_at, collection, record_id, field LIMIT 80",
        )
        .fetch_all(&store.0)
        .await?;
        let mut grouped = BTreeMap::<(&str, &str), Vec<&Job>>::new();
        for job in &jobs {
            grouped
                .entry((&job.collection, &job.record_id))
                .or_default()
                .push(job);
        }
        for ((collection, id), jobs) in grouped {
            let Some(kind) = Kind::parse(collection) else {
                continue;
            };
            let (mut base, mut artist, mut lyrics, mut unsupported) =
                (Vec::new(), Vec::new(), Vec::new(), Vec::new());
            for job in jobs {
                match source_for(kind, &job.field) {
                    Some(Provider::MusicBrainz) => base.push(job),
                    Some(Provider::Wikidata) => artist.push(job),
                    Some(Provider::Lrclib) => lyrics.push(job),
                    None => unsupported.push(job),
                }
            }
            if !base.is_empty() {
                let result = self.base(store, kind, id, &base).await;
                finish(store, base, result).await?;
            }
            if !artist.is_empty() {
                let fields = artist
                    .iter()
                    .map(|job| job.field.as_str())
                    .collect::<Vec<_>>();
                let result =
                    wikidata::enrich(&self.wikidata, &self.musicbrainz, store, id, &fields).await;
                match result {
                    Ok(file) => {
                        let (artwork, metadata) =
                            artist.into_iter().partition(|job| job.field == "artwork");
                        finish(store, metadata, Ok(())).await?;
                        let result = if let Some(file) = file {
                            wikimedia_commons::enrich(&self.commons, store, id, &file)
                                .await
                                .map(|_| ())
                        } else {
                            Ok(())
                        };
                        finish(store, artwork, result).await?;
                    }
                    Err(error) => finish(store, artist, Err(error)).await?,
                }
            }
            if let Some(job) = lyrics.first() {
                let result = lrclib::enrich(&self.lrclib, store, id, job.refresh, job.generation)
                    .await
                    .map(|_| ());
                finish(store, lyrics, result).await?;
            }
            finish(store, unsupported, Ok(())).await?;
        }
        Ok(())
    }

    async fn base(&self, store: &Store, kind: Kind, id: &str, jobs: &[&Job]) -> Result<()> {
        let Some(table) = kind.external_id_table() else {
            return Ok(());
        };
        let statement = format!(
            "SELECT value FROM {table} WHERE owner_id = ? AND provider = 'Musicbrainz' ORDER BY value LIMIT 1"
        );
        let mbid: Option<String> = query_scalar(AssertSqlSafe(statement))
            .bind(id)
            .fetch_optional(&store.0)
            .await?;
        let Some(mbid) = mbid.and_then(|value| value.parse().ok()) else {
            return Ok(());
        };
        let batch = self.musicbrainz.entity(kind, mbid).await?;
        let mut db = store.0.begin_with("BEGIN IMMEDIATE").await?;
        let mut fields = BTreeMap::new();
        for job in jobs {
            let active: bool = query_scalar(
                "SELECT EXISTS(SELECT 1 FROM enrichment_needed WHERE
                collection = ? AND record_id = ? AND field = ? AND generation = ?)",
            )
            .bind(kind.path())
            .bind(id)
            .bind(&job.field)
            .bind(job.generation)
            .fetch_one(&mut *db)
            .await?;
            if active {
                fields.insert(
                    job.field.as_str(),
                    if job.refresh {
                        proposed(&batch, &job.field)
                    } else {
                        None
                    },
                );
            }
        }
        if !fields.is_empty() {
            let before = editor::snapshot(&mut db, kind, id).await?;
            import::apply(&mut db, batch).await?;
            editor::reconcile_source(&mut db, kind, id, &before, &fields).await?;
        }
        db.commit().await?;
        Ok(())
    }
}

async fn finish(store: &Store, jobs: Vec<&Job>, result: Result<()>) -> Result<()> {
    let error = result.err().filter(|error| {
        error
            .downcast_ref::<reqwest::Error>()
            .and_then(reqwest::Error::status)
            != Some(reqwest::StatusCode::NOT_FOUND)
    });
    let sql = error.map_or(
        "DELETE FROM enrichment_needed WHERE collection = ? AND record_id = ? AND field = ? AND generation = ?",
        |error| {
            eprintln!("Enrichment attempt deferred: {error}");
            "UPDATE enrichment_needed SET due_at = unixepoch() + 60 WHERE collection = ? AND record_id = ? AND field = ? AND generation = ?"
        },
    );
    for job in jobs {
        query(sql)
            .bind(&job.collection)
            .bind(&job.record_id)
            .bind(&job.field)
            .bind(job.generation)
            .execute(&store.0)
            .await?;
    }
    Ok(())
}

enum Provider {
    MusicBrainz,
    Wikidata,
    Lrclib,
}

fn source_for(kind: Kind, field: &str) -> Option<Provider> {
    match (kind, field) {
        (Kind::Artist, "artwork" | "biography" | "begin_date" | "end_date" | "external_ids") => {
            Some(Provider::Wikidata)
        }
        (Kind::Track, "lyrics") => Some(Provider::Lrclib),
        (Kind::Artist, "name" | "sort_name" | "kind" | "origin" | "tags")
        | (
            Kind::Album,
            "title" | "album_type" | "release_date" | "label" | "discs" | "credits"
            | "external_ids" | "tags" | "artwork",
        )
        | (
            Kind::Track,
            "title" | "duration_ms" | "release_date" | "album_id" | "external_ids" | "credits"
            | "tags",
        ) => Some(Provider::MusicBrainz),
        _ => None,
    }
}

fn proposed(batch: &Batch, field: &str) -> Option<Value> {
    match batch {
        Batch::Artists(values) => {
            let artist = values.first()?;
            match field {
                "name" => Some(json!(artist.name)),
                "sort_name" => Some(json!(artist.sort_name)),
                "origin" => Some(json!(
                    artist
                        .begin_area
                        .as_ref()
                        .or(artist.area.as_ref())
                        .map(|area| &area.name)
                )),
                _ => None,
            }
        }
        Batch::Releases(values) => {
            let album = values.first()?;
            match field {
                "title" => Some(json!(album.title)),
                "release_date" => Some(json!(
                    album
                        .date
                        .as_deref()?
                        .parse::<concentus_types::PartialDate>()
                        .ok()?
                        .to_string()
                )),
                "label" => Some(json!(
                    album
                        .labels
                        .iter()
                        .find_map(|info| info.label.as_ref())
                        .map(|label| &label.name)
                )),
                _ => None,
            }
        }
        Batch::Recordings(values) => {
            let track = values.first()?;
            match field {
                "title" => Some(json!(track.title)),
                "duration_ms" => Some(json!(track.length.filter(|length| *length > 0))),
                "release_date" => Some(json!(
                    track
                        .date
                        .as_deref()?
                        .parse::<concentus_types::PartialDate>()
                        .ok()?
                        .to_string()
                )),
                _ => None,
            }
        }
    }
}
