use super::{Enrichment, import, lrclib, musicbrainz::Batch, wikidata, wikimedia_commons};
use crate::{catalog::Kind, editor, store::Store};
use serde_json::{Value, json};
use sqlx::{query, query_as, query_scalar};
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
        tracing::info!(interval_seconds = 60, "Enrichment worker started");
        tokio::spawn(async move {
            let mut ticker = interval(Duration::from_secs(60));
            ticker.set_missed_tick_behavior(MissedTickBehavior::Skip);
            // Let the server start accepting requests before the first pass.
            ticker.tick().await;
            loop {
                tokio::select! { _ = ticker.tick() => {}, () = self.wake.notified() => {} }
                if let Err(error) = self.work(&store).await {
                    tracing::error!(%error, "Enrichment pass failed");
                }
            }
        });
    }

    async fn work(&self, store: &Store) -> Result<()> {
        let jobs: Vec<Job> = query_as(
            "SELECT records.collection, queue.record_id, queue.field, queue.generation, queue.refresh
            FROM enrichment_needed AS queue JOIN records ON records.id = queue.record_id
            WHERE due_at <= unixepoch() ORDER BY refresh DESC, due_at, collection, record_id, field LIMIT 80",
        )
        .fetch_all(&store.0)
        .await?;
        if jobs.is_empty() {
            tracing::debug!("No enrichment jobs due");
            return Ok(());
        }
        tracing::info!(jobs = jobs.len(), "Processing enrichment queue");
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
            let (discography, fields): (Vec<_>, Vec<_>) = jobs
                .into_iter()
                .partition(|job| kind == Kind::Artist && job.field == "discography");
            let (mut base, mut artist, mut lyrics, mut unsupported) =
                (Vec::new(), Vec::new(), Vec::new(), Vec::new());
            for job in fields {
                match source_for(kind, &job.field) {
                    Some(Provider::MusicBrainz) => base.push(job),
                    Some(Provider::Wikidata) => artist.push(job),
                    Some(Provider::Lrclib) => lyrics.push(job),
                    None => unsupported.push(job),
                }
            }
            if !base.is_empty() {
                let result = self.base(store, kind, id, &base).await.map(|()| true);
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
                        let (artwork, metadata) = artist
                            .into_iter()
                            .partition(|job| job.field == "artwork_portrait");
                        finish(store, metadata, Ok(true)).await?;
                        let result = if let Some(file) = file {
                            wikimedia_commons::enrich(&self.commons, store, id, &file).await
                        } else {
                            Ok(false)
                        };
                        finish(store, artwork, result).await?;
                    }
                    Err(error) => finish(store, artist, Err(error)).await?,
                }
            }
            if let Some(job) = lyrics.first() {
                let result =
                    lrclib::enrich(&self.lrclib, store, id, job.refresh, job.generation).await;
                finish(store, lyrics, result).await?;
            }
            finish(store, unsupported, Ok(false)).await?;
            if let Some(job) = discography.first() {
                let result = self.discography(store, job).await.map(|()| true);
                finish(store, discography, result).await?;
            }
        }
        Ok(())
    }

    async fn discography(&self, store: &Store, job: &Job) -> Result<()> {
        let Some(mbid) = musicbrainz_id(store, &job.record_id).await? else {
            return Ok(());
        };
        let releases = self.musicbrainz.albums_by_artist(mbid).await?;
        tracing::debug!(
            record_id = job.record_id,
            releases = releases.len(),
            "Fetched artist discography"
        );
        let mut db = store.0.begin_with("BEGIN IMMEDIATE").await?;
        if active(&mut db, job).await? {
            import::albums(&mut db, &job.record_id, releases).await?;
        }
        db.commit().await?;
        Ok(())
    }

    async fn base(&self, store: &Store, kind: Kind, id: &str, jobs: &[&Job]) -> Result<()> {
        let Some(mbid) = musicbrainz_id(store, id).await? else {
            return Ok(());
        };
        let batch = self.musicbrainz.entity(kind, mbid).await?;
        let mut db = store.0.begin_with("BEGIN IMMEDIATE").await?;
        let mut fields = BTreeMap::new();
        for job in jobs {
            if active(&mut db, job).await? {
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

async fn musicbrainz_id(store: &Store, id: &str) -> Result<Option<uuid::Uuid>> {
    let value: Option<String> = query_scalar(
        "SELECT value FROM external_ids WHERE owner_id = ? AND provider = 'Musicbrainz' ORDER BY value LIMIT 1",
    )
    .bind(id)
    .fetch_optional(&store.0)
    .await?;
    Ok(value.and_then(|value| value.parse().ok()))
}

async fn active(db: &mut sqlx::SqliteConnection, job: &Job) -> Result<bool> {
    Ok(query_scalar(
        "SELECT EXISTS(SELECT 1 FROM enrichment_needed WHERE
        record_id = ? AND field = ? AND generation = ?)",
    )
    .bind(&job.record_id)
    .bind(&job.field)
    .bind(job.generation)
    .fetch_one(db)
    .await?)
}

async fn finish(store: &Store, jobs: Vec<&Job>, result: Result<bool>) -> Result<()> {
    if jobs.is_empty() {
        return Ok(());
    }
    let outcome = match result {
        Ok(true) => "Checked",
        Ok(false) => "No new data",
        Err(error)
            if error
                .downcast_ref::<reqwest::Error>()
                .and_then(reqwest::Error::status)
                == Some(reqwest::StatusCode::NOT_FOUND) =>
        {
            "No result"
        }
        Err(error) => {
            let job = jobs[0];
            tracing::warn!(collection = job.collection, record_id = job.record_id, fields = jobs.len(), %error, retry_seconds = 60, "Enrichment retry scheduled");
            let mut db = store.0.begin().await?;
            let description = error.to_string();
            for job in jobs {
                query(
                    "UPDATE enrichment_needed SET due_at = unixepoch() + 60, last_error = ?
                    WHERE record_id = ? AND field = ? AND generation = ?",
                )
                .bind(&description)
                .bind(&job.record_id)
                .bind(&job.field)
                .bind(job.generation)
                .execute(&mut *db)
                .await?;
            }
            db.commit().await?;
            return Ok(());
        }
    };
    let mut db = store.0.begin().await?;
    for job in jobs {
        let provider = match (
            Kind::parse(&job.collection).and_then(|kind| source_for(kind, &job.field)),
            job.field.as_str(),
        ) {
            (Some(Provider::MusicBrainz), _) | (None, "discography") => Some("MusicBrainz"),
            (Some(Provider::Wikidata), "artwork_portrait") => Some("Wikidata / Wikimedia Commons"),
            (Some(Provider::Wikidata), _) => Some("Wikidata"),
            (Some(Provider::Lrclib), _) => Some("LRCLIB"),
            (None, _) => None,
        };
        let removed = query(
            "DELETE FROM enrichment_needed WHERE record_id = ? AND field = ? AND generation = ?",
        )
        .bind(&job.record_id)
        .bind(&job.field)
        .bind(job.generation)
        .execute(&mut *db)
        .await?;
        if let Some(provider) = provider
            && removed.rows_affected() > 0
        {
            query("INSERT INTO record_history (record_id, field, minute, old_value, new_value, checked_at, check_source, outcome)
                   VALUES (?, ?, unixepoch() / 60, 'null', 'null', unixepoch(), ?, ?)
                   ON CONFLICT (record_id, field, minute) DO UPDATE SET
                       checked_at = excluded.checked_at, check_source = excluded.check_source, outcome = excluded.outcome")
                .bind(&job.record_id).bind(&job.field).bind(provider).bind(outcome).execute(&mut *db).await?;
        }
    }
    db.commit().await?;
    Ok(())
}

enum Provider {
    MusicBrainz,
    Wikidata,
    Lrclib,
}

pub fn can_refresh(kind: Kind, field: &str) -> bool {
    source_for(kind, field).is_some()
}

fn source_for(kind: Kind, field: &str) -> Option<Provider> {
    match (kind, field) {
        (
            Kind::Artist,
            "artwork_portrait" | "biography" | "begin_date" | "end_date" | "external_ids",
        ) => Some(Provider::Wikidata),
        (Kind::Track, "lyrics") => Some(Provider::Lrclib),
        (Kind::Artist, "name" | "sort_name" | "kind" | "origin" | "tags")
        | (
            Kind::Album,
            "title" | "album_type" | "release_date" | "label" | "discs" | "credits"
            | "external_ids" | "tags" | "artwork_square",
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
