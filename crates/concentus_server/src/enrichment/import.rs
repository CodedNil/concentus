use super::{
    cover_art_archive::{RELEASE_GROUP_PROVIDER, front},
    musicbrainz::{Artist, Batch, Credit, Recording, Release, SourceTag},
};
use crate::{catalog::Kind, store::Store};
use concentus_types::{Id, Isrc, PartialDate};
use sqlx::{AssertSqlSafe, SqliteConnection, query, query_scalar};
use topcoat::Result;
use uuid::Uuid;

pub async fn save(store: &Store, batch: Batch) -> Result<Vec<String>> {
    // Serialize identity resolution and writes across requests and processes.
    let mut db = store.0.begin_with("BEGIN IMMEDIATE").await?;
    let mut ids = Vec::new();
    match batch {
        Batch::Artists(artists) => {
            for value in artists {
                ids.push(artist(&mut db, &value).await?);
            }
        }
        Batch::Releases(releases) => {
            for value in releases {
                ids.push(release(&mut db, &value, true).await?);
            }
        }
        Batch::Recordings(recordings) => {
            for value in recordings {
                let primary = value.releases.iter().min_by_key(|release| {
                    (
                        release.status.as_deref() != Some("Official"),
                        release
                            .group
                            .as_ref()
                            .and_then(|group| group.primary_type.as_deref())
                            != Some("Album"),
                        date(release.date.as_deref()).unwrap_or(PartialDate::Year(u16::MAX)),
                    )
                });
                let album = match primary {
                    Some(value) => Some(release(&mut db, value, false).await?),
                    None => None,
                };
                ids.push(recording(&mut db, &value, album.as_deref(), None, None).await?);
            }
        }
    }
    db.commit().await?;
    Ok(ids)
}

fn date(value: Option<&str>) -> Option<PartialDate> {
    value.and_then(|value| value.parse().ok())
}

fn date_text(value: Option<&str>) -> Option<String> {
    date(value).map(|value| value.to_string())
}

async fn identity(db: &mut SqliteConnection, kind: Kind, mbid: &str) -> Result<String> {
    let normalized: Id<()> = mbid.parse()?;
    let sql = format!(
        "SELECT owner_id FROM {} WHERE provider = 'Musicbrainz' AND value = ? ORDER BY owner_id LIMIT 1",
        kind.external_id_table()
            .expect("MusicBrainz entities have external IDs")
    );
    Ok(query_scalar(AssertSqlSafe(sql))
        .bind(normalized.to_string())
        .fetch_optional(db)
        .await?
        .unwrap_or_else(|| Uuid::now_v7().to_string()))
}

async fn external(
    db: &mut SqliteConnection,
    kind: Kind,
    id: &str,
    provider: &str,
    value: &str,
) -> Result<()> {
    if !value.trim().is_empty() {
        let sql = format!(
            "INSERT INTO {} (owner_id, provider, value) VALUES (?, ?, ?) ON CONFLICT DO NOTHING",
            kind.external_id_table()
                .expect("MusicBrainz entities have external IDs")
        );
        query(AssertSqlSafe(sql))
            .bind(id)
            .bind(provider)
            .bind(value)
            .execute(db)
            .await?;
    }
    Ok(())
}

async fn artist(db: &mut SqliteConnection, artist: &Artist) -> Result<String> {
    let id = identity(db, Kind::Artist, &artist.id).await?;
    let kind = match artist.kind.as_deref() {
        Some("Person") => "Person",
        Some("Group" | "Orchestra" | "Choir") => "Group",
        _ => "Unknown",
    };
    let begin = artist
        .lifespan
        .as_ref()
        .and_then(|span| span.begin.as_deref());
    let end = artist
        .lifespan
        .as_ref()
        .and_then(|span| span.end.as_deref());
    let origin = artist
        .begin_area
        .as_ref()
        .or(artist.area.as_ref())
        .map(|area| &area.name);
    query(
        "INSERT INTO artists (id, name, sort_name, kind, begin_date, end_date, origin)
        VALUES (?, ?, ?, ?, ?, ?, ?)
        ON CONFLICT(id) DO UPDATE SET
            sort_name = coalesce(artists.sort_name, excluded.sort_name),
            kind = CASE WHEN artists.kind = 'Unknown' THEN excluded.kind ELSE artists.kind END,
            begin_date = coalesce(artists.begin_date, excluded.begin_date),
            end_date = coalesce(artists.end_date, excluded.end_date),
            origin = coalesce(artists.origin, excluded.origin)",
    )
    .bind(&id)
    .bind(&artist.name)
    .bind(&artist.sort_name)
    .bind(kind)
    .bind(date_text(begin))
    .bind(date_text(end))
    .bind(origin)
    .execute(&mut *db)
    .await?;
    external(db, Kind::Artist, &id, "Musicbrainz", &artist.id).await?;
    for value in &artist.isnis {
        external(db, Kind::Artist, &id, "Isni", value).await?;
    }
    for value in &artist.ipis {
        external(db, Kind::Artist, &id, "Ipi", value).await?;
    }
    tags(db, Kind::Artist, &id, &artist.tags).await?;
    Ok(id)
}

async fn release(
    db: &mut SqliteConnection,
    release: &Release,
    include_tracks: bool,
) -> Result<String> {
    let id = identity(db, Kind::Album, &release.id).await?;
    let album_type = release.group.as_ref().and_then(|group| {
        if group.secondary_types.iter().any(|ty| ty == "Soundtrack") {
            return Some("Soundtrack");
        }
        group.primary_type.as_deref().map(|ty| match ty {
            "Album" => "Album",
            "Single" => "Single",
            "EP" => "Ep",
            _ => "Other",
        })
    });
    let label = release
        .labels
        .iter()
        .find_map(|info| info.label.as_ref().map(|label| &label.name));
    query(
        "INSERT INTO albums (id, title, album_type, release_date, label) VALUES (?, ?, ?, ?, ?)
        ON CONFLICT(id) DO UPDATE SET
            album_type = coalesce(albums.album_type, excluded.album_type),
            release_date = coalesce(albums.release_date, excluded.release_date),
            label = coalesce(albums.label, excluded.label)",
    )
    .bind(&id)
    .bind(&release.title)
    .bind(album_type)
    .bind(date_text(release.date.as_deref()))
    .bind(label)
    .execute(&mut *db)
    .await?;
    external(db, Kind::Album, &id, "Musicbrainz", &release.id).await?;
    if let Some(group) = release.group.as_ref().and_then(|group| group.id.as_deref()) {
        external(
            db,
            Kind::Album,
            &id,
            &format!("other:{RELEASE_GROUP_PROVIDER}"),
            group,
        )
        .await?;
    }
    if let Some(barcode) = &release.barcode {
        external(db, Kind::Album, &id, "Barcode", barcode).await?;
    }
    for number in release
        .labels
        .iter()
        .filter_map(|info| info.catalog_number.as_deref())
    {
        external(db, Kind::Album, &id, "other:Catalog number", number).await?;
    }
    credits(db, Kind::Album, &id, &release.credits).await?;
    tags(db, Kind::Album, &id, &release.tags).await?;
    if release.cover_art.as_ref().is_some_and(|art| art.front)
        && let Some(url) = front(&release.id, false)
    {
        let artwork: i64 = query_scalar("INSERT INTO album_artwork (owner_id, position, kind, explicit)
            SELECT ?, coalesce(max(position) + 1, 0), 'Cover', 0 FROM album_artwork WHERE owner_id = ?
            HAVING NOT EXISTS (SELECT 1 FROM album_artwork WHERE owner_id = ? AND kind = 'Cover')
            RETURNING id")
            .bind(&id).bind(&id).bind(&id)
            .fetch_optional(&mut *db).await?.unwrap_or_default();
        if artwork != 0 {
            query(
                "INSERT INTO album_artwork_renditions (owner_id, position, url, width, height)
                VALUES (?, 0, ?, 500, 500)",
            )
            .bind(artwork)
            .bind(url)
            .execute(&mut *db)
            .await?;
        }
    }
    if include_tracks {
        for medium in &release.media {
            let Some(position) = medium.position.filter(|position| *position > 0) else {
                continue;
            };
            let disc_id: i64 = query_scalar("INSERT INTO album_discs (owner_id, position, title) VALUES (?, ?, ?)
                ON CONFLICT(owner_id, position) DO UPDATE SET title = coalesce(album_discs.title, excluded.title)
                RETURNING id")
                .bind(&id).bind(position).bind(medium.title.as_deref().filter(|title| !title.is_empty()))
                .fetch_one(&mut *db).await?;
            for track in &medium.tracks {
                let track_id = recording(
                    db,
                    &track.recording,
                    Some(&id),
                    track.length,
                    Some(&track.credits),
                )
                .await?;
                external(
                    db,
                    Kind::Track,
                    &track_id,
                    "other:MusicBrainz release track",
                    &track.id,
                )
                .await?;
                query(
                    "INSERT INTO disc_tracks (owner_id, position, track_id) VALUES (?, ?, ?)
                    ON CONFLICT DO NOTHING",
                )
                .bind(disc_id)
                .bind(track.position)
                .bind(track_id)
                .execute(&mut *db)
                .await?;
            }
        }
    }
    Ok(id)
}

async fn recording(
    db: &mut SqliteConnection,
    recording: &Recording,
    album: Option<&str>,
    length: Option<u32>,
    alternate_credits: Option<&[Credit]>,
) -> Result<String> {
    let id = identity(db, Kind::Track, &recording.id).await?;
    query("INSERT INTO tracks (id, title, duration_ms, album_id, release_date) VALUES (?, ?, ?, ?, ?)
        ON CONFLICT(id) DO UPDATE SET
            album_id = coalesce(tracks.album_id, excluded.album_id),
            duration_ms = CASE WHEN tracks.duration_ms = 0 THEN excluded.duration_ms ELSE tracks.duration_ms END,
            release_date = coalesce(tracks.release_date, excluded.release_date)")
        .bind(&id).bind(&recording.title).bind(recording.length.or(length).unwrap_or_default())
        .bind(album).bind(date_text(recording.date.as_deref())).execute(&mut *db).await?;
    external(db, Kind::Track, &id, "Musicbrainz", &recording.id).await?;
    for raw in &recording.isrcs {
        if let Ok(value) = raw.parse::<Isrc>() {
            external(db, Kind::Track, &id, "Isrc", value.as_str()).await?;
        }
    }
    let billed = if recording.credits.is_empty() {
        alternate_credits.unwrap_or_default()
    } else {
        &recording.credits
    };
    credits(db, Kind::Track, &id, billed).await?;
    tags(db, Kind::Track, &id, &recording.tags).await?;
    Ok(id)
}

async fn credits(
    db: &mut SqliteConnection,
    kind: Kind,
    id: &str,
    credits: &[Credit],
) -> Result<()> {
    let Some(table) = kind.credit_table() else {
        return Ok(());
    };
    let sql = format!(
        "INSERT INTO {table} (owner_id, position, artist_id, role) VALUES (?, ?, ?, 'Primary')
        ON CONFLICT DO NOTHING"
    );
    for (position, credit) in credits.iter().enumerate() {
        let artist = artist(db, &credit.artist).await?;
        query(AssertSqlSafe(sql.as_str()))
            .bind(id)
            .bind(i64::try_from(position)?)
            .bind(artist)
            .execute(&mut *db)
            .await?;
    }
    Ok(())
}

async fn tags(db: &mut SqliteConnection, kind: Kind, id: &str, tags: &[SourceTag]) -> Result<()> {
    let Some(table) = kind.tag_table() else {
        return Ok(());
    };
    for tag in tags.iter().filter(|tag| !tag.name.trim().is_empty()) {
        // MusicBrainz provides vote counts rather than a calibrated weight.
        let weight = tag.count.unwrap_or(1).clamp(0, 100);
        if weight == 0 {
            continue;
        }
        let existing: Option<String> = query_scalar("SELECT id FROM tags WHERE trim(name) = trim(?) COLLATE NOCASE AND kind = 'Genre' ORDER BY id LIMIT 1")
            .bind(&tag.name).fetch_optional(&mut *db).await?;
        let tag_id = if let Some(id) = existing {
            id
        } else {
            let id = Uuid::now_v7().to_string();
            query("INSERT INTO tags (id, name, kind) VALUES (?, ?, 'Genre')")
                .bind(&id)
                .bind(&tag.name)
                .execute(&mut *db)
                .await?;
            id
        };
        let sql = format!(
            "INSERT INTO {table} (owner_id, position, tag_id, weight)
            SELECT ?, coalesce(max(position) + 1, 0), ?, ? FROM {table} WHERE owner_id = ?
            HAVING NOT EXISTS (SELECT 1 FROM {table} WHERE owner_id = ? AND tag_id = ?)"
        );
        query(AssertSqlSafe(sql))
            .bind(id)
            .bind(&tag_id)
            .bind(weight)
            .bind(id)
            .bind(id)
            .bind(&tag_id)
            .execute(&mut *db)
            .await?;
    }
    Ok(())
}
