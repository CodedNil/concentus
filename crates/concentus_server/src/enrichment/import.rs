use super::{
    cover_art_archive::{RELEASE_GROUP_PROVIDER, front},
    musicbrainz::{Artist, Batch, Credit, Recording, Release, SourceTag},
};
use crate::editor;
use crate::{catalog::Kind, store::Store};
use concentus_types::{Id, Isrc, PartialDate};
use sqlx::{SqliteConnection, query, query_scalar};
use topcoat::Result;
use uuid::Uuid;

pub async fn albums(
    db: &mut SqliteConnection,
    artist_id: &str,
    mut releases: Vec<Release>,
) -> Result<()> {
    let group_provider = format!("other:{RELEASE_GROUP_PROVIDER}");
    let release_date =
        |release: &Release| date(release.date.as_deref()).unwrap_or(PartialDate::Year(u16::MAX));
    releases.sort_by(|a, b| {
        release_date(a)
            .cmp(&release_date(b))
            .then_with(|| a.id.cmp(&b.id))
    });
    let mut groups = std::collections::HashSet::new();
    for value in &releases {
        if let Some(group) = value.group.as_ref().and_then(|group| group.id.as_deref()) {
            if !groups.insert(group) {
                continue;
            }
            let already_credited: bool = query_scalar(
                "SELECT EXISTS(SELECT 1 FROM external_ids AS external
                 JOIN albums ON albums.id = external.owner_id
                 JOIN credits ON credits.owner_id = albums.id
                 WHERE external.provider = ? AND external.value = ? AND credits.artist_id = ?)",
            )
            .bind(&group_provider)
            .bind(group)
            .bind(artist_id)
            .fetch_one(&mut *db)
            .await?;
            if already_credited {
                continue;
            }
        }
        release(db, value, false).await?;
    }
    Ok(())
}

pub async fn save(store: &Store, batch: Batch) -> Result<Vec<String>> {
    // Serialize identity resolution and writes across requests and processes.
    let mut db = store.0.begin_with("BEGIN IMMEDIATE").await?;
    let ids = apply(&mut db, batch).await?;
    db.commit().await?;
    Ok(ids)
}

pub async fn apply(db: &mut SqliteConnection, batch: Batch) -> Result<Vec<String>> {
    let mut ids = Vec::new();
    match batch {
        Batch::Artists(artists) => {
            for value in artists {
                ids.push(artist(db, &value).await?);
            }
        }
        Batch::Releases(releases) => {
            for value in releases {
                ids.push(release(db, &value, true).await?);
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
                    Some(value) => Some(release(db, value, false).await?),
                    None => None,
                };
                ids.push(recording(db, &value, album.as_deref(), None, None).await?);
            }
        }
    }
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
    let old: Option<String> = query_scalar(
        "SELECT external.owner_id FROM external_ids AS external
         JOIN records ON records.id = external.owner_id
         WHERE external.provider = 'Musicbrainz' AND external.value = ? AND records.collection = ?
         ORDER BY external.owner_id LIMIT 1",
    )
    .bind(normalized.to_string())
    .bind(kind.path())
    .fetch_optional(&mut *db)
    .await?;
    if let Some(id) = old {
        return Ok(id);
    }
    let id = Uuid::now_v7().to_string();
    query("INSERT INTO records (id, collection) VALUES (?, ?)")
        .bind(&id)
        .bind(kind.path())
        .execute(db)
        .await?;
    Ok(id)
}

async fn external(db: &mut SqliteConnection, id: &str, provider: &str, value: &str) -> Result<()> {
    if !value.trim().is_empty() {
        query("INSERT INTO external_ids (owner_id, provider, value) VALUES (?, ?, ?) ON CONFLICT DO NOTHING")
            .bind(id).bind(provider).bind(value).execute(db).await?;
    }
    Ok(())
}

async fn artist(db: &mut SqliteConnection, artist: &Artist) -> Result<String> {
    let id = identity(db, Kind::Artist, &artist.id).await?;
    let before = editor::snapshot(db, Kind::Artist, &id).await?;
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
    external(db, &id, "Musicbrainz", &artist.id).await?;
    for value in &artist.isnis {
        external(db, &id, "Isni", value).await?;
    }
    for value in &artist.ipis {
        external(db, &id, "Ipi", value).await?;
    }
    tags(db, &id, &artist.tags).await?;
    if before.is_empty() {
        editor::queue(db, &id, "discography", false).await?;
    }
    editor::finish_import(db, Kind::Artist, &id, &before).await?;
    Ok(id)
}

async fn release(
    db: &mut SqliteConnection,
    release: &Release,
    include_tracks: bool,
) -> Result<String> {
    let id = identity(db, Kind::Album, &release.id).await?;
    let before = editor::snapshot(db, Kind::Album, &id).await?;
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
    external(db, &id, "Musicbrainz", &release.id).await?;
    if let Some(group) = release.group.as_ref().and_then(|group| group.id.as_deref()) {
        external(db, &id, &format!("other:{RELEASE_GROUP_PROVIDER}"), group).await?;
    }
    if let Some(barcode) = &release.barcode {
        external(db, &id, "Barcode", barcode).await?;
    }
    for number in release
        .labels
        .iter()
        .filter_map(|info| info.catalog_number.as_deref())
    {
        external(db, &id, "other:Catalog number", number).await?;
    }
    credits(db, &id, &release.credits).await?;
    tags(db, &id, &release.tags).await?;
    if release.cover_art.as_ref().is_some_and(|art| art.front)
        && let Some(url) = front(&release.id, false)
    {
        let artwork: i64 = query_scalar(
            "INSERT INTO artwork (owner_id, kind, explicit) VALUES (?, 'Square', 0)
            ON CONFLICT (owner_id, kind) DO NOTHING
            RETURNING id",
        )
        .bind(&id)
        .fetch_optional(&mut *db)
        .await?
        .unwrap_or_default();
        if artwork != 0 {
            query(
                "INSERT INTO artwork_renditions (owner_id, position, url, width, height)
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
                external(db, &track_id, "other:MusicBrainz release track", &track.id).await?;
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
    editor::finish_import(db, Kind::Album, &id, &before).await?;
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
    let before = editor::snapshot(db, Kind::Track, &id).await?;
    query("INSERT INTO tracks (id, title, duration_ms, album_id, release_date) VALUES (?, ?, ?, ?, ?)
        ON CONFLICT(id) DO UPDATE SET
            album_id = coalesce(tracks.album_id, excluded.album_id),
            duration_ms = CASE WHEN tracks.duration_ms = 0 THEN excluded.duration_ms ELSE tracks.duration_ms END,
            release_date = coalesce(tracks.release_date, excluded.release_date)")
        .bind(&id).bind(&recording.title).bind(recording.length.or(length).unwrap_or_default())
        .bind(album).bind(date_text(recording.date.as_deref())).execute(&mut *db).await?;
    external(db, &id, "Musicbrainz", &recording.id).await?;
    for raw in &recording.isrcs {
        if let Ok(value) = raw.parse::<Isrc>() {
            external(db, &id, "Isrc", value.as_str()).await?;
        }
    }
    let billed = if recording.credits.is_empty() {
        alternate_credits.unwrap_or_default()
    } else {
        &recording.credits
    };
    credits(db, &id, billed).await?;
    tags(db, &id, &recording.tags).await?;
    editor::finish_import(db, Kind::Track, &id, &before).await?;
    Ok(id)
}

async fn credits(db: &mut SqliteConnection, id: &str, credits: &[Credit]) -> Result<()> {
    for (position, credit) in credits.iter().enumerate() {
        let artist = artist(db, &credit.artist).await?;
        query("INSERT INTO credits (owner_id, position, artist_id, role) VALUES (?, ?, ?, 'Primary') ON CONFLICT DO NOTHING")
            .bind(id)
            .bind(i64::try_from(position)?)
            .bind(artist)
            .execute(&mut *db)
            .await?;
    }
    Ok(())
}

async fn tags(db: &mut SqliteConnection, id: &str, tags: &[SourceTag]) -> Result<()> {
    for tag in tags.iter().filter(|tag| !tag.name.trim().is_empty()) {
        // MusicBrainz provides vote counts rather than a calibrated weight.
        let weight = tag.count.unwrap_or(1).clamp(0, 100);
        if weight == 0 {
            continue;
        }
        let existing: Option<String> = query_scalar("SELECT CASE WHEN relation_kind = 'AliasOf' THEN related_id ELSE id END FROM tags WHERE trim(name) = trim(?) COLLATE NOCASE ORDER BY kind = 'Unclassified', id LIMIT 1")
            .bind(&tag.name).fetch_optional(&mut *db).await?;
        let tag_id = if let Some(id) = existing {
            id
        } else {
            let id = Uuid::now_v7().to_string();
            query("INSERT INTO tags (id, name, kind) VALUES (?, ?, 'Unclassified')")
                .bind(&id)
                .bind(&tag.name)
                .execute(&mut *db)
                .await?;
            id
        };
        query(
            "INSERT INTO record_tags (owner_id, tag_id, weight)
             VALUES (?, ?, ?) ON CONFLICT (owner_id, tag_id) DO NOTHING",
        )
        .bind(id)
        .bind(&tag_id)
        .bind(weight)
        .execute(&mut *db)
        .await?;
    }
    Ok(())
}
