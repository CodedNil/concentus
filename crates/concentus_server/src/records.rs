//! Maps normalized SQLite rows to the shared metadata types.
use concentus_types::{
    Album, AlbumArtworkKind, AlbumType, Artist, ArtistArtworkKind, ArtistKind, Artwork,
    AudioFeatures, Channel, Collection, Credit, DerivationKind, Disc, Edition, ExternalIdProvider,
    ExternalIds, Font, Id, LookupOptions, Lyrics, Membership, RecordRef, RelatedData, Rendition,
    Role, Scheme, Scored, Span, Tag, TagRelation, TimeRange, Track, TrackArtworkKind, VisualData,
};
use serde::de::{DeserializeOwned, value::StrDeserializer};
use sqlx::{AssertSqlSafe, Row, Sqlite, SqliteConnection, query, sqlite::SqliteRow};
use std::{collections::HashMap, future::Future, str::FromStr};
use topcoat::{Error, Result};

pub trait DatabaseRecord: Collection + Send {
    const COLUMNS: &'static str = "record.*";
    const HAS_CREDITS: bool = false;
    const HAS_EXTERNAL_IDS: bool = false;

    fn read(db: &mut SqliteConnection, row: SqliteRow)
    -> impl Future<Output = Result<Self>> + Send;

    fn load_lyrics(
        &mut self,
        _db: &mut SqliteConnection,
    ) -> impl Future<Output = Result<()>> + Send {
        async { Ok(()) }
    }

    fn omit_lyrics(&mut self) {}

    fn select_relations(
        &mut self,
        _db: &mut SqliteConnection,
        _options: LookupOptions,
    ) -> impl Future<Output = Result<()>> + Send {
        async { Ok(()) }
    }
}

pub async fn read_by_id<T: DatabaseRecord>(
    db: &mut SqliteConnection,
    id: &str,
) -> Result<Option<T>> {
    let sql = format!(
        "SELECT {} FROM {} AS record WHERE id = ?",
        T::COLUMNS,
        T::NAME
    );
    let row = query(AssertSqlSafe(sql))
        .bind(id)
        .fetch_optional(&mut *db)
        .await?;
    match row {
        Some(row) => Ok(Some(T::read(db, row).await?)),
        None => Ok(None),
    }
}

impl DatabaseRecord for Artist {
    const HAS_EXTERNAL_IDS: bool = true;

    async fn read(db: &mut SqliteConnection, row: SqliteRow) -> Result<Self> {
        let owner = Key::Text(row.try_get("id")?);
        let kind = match row.try_get::<&str, _>("kind")? {
            "Person" => ArtistKind::Person,
            "Group" => {
                let mut members = Vec::new();
                for member in children(db, "artist_members", owner).await? {
                    members.push(Membership {
                        artist_id: parse(&member, "artist_id")?,
                        roles: ordered(
                            db,
                            "member_roles",
                            Key::Number(member.try_get("id")?),
                            role,
                        )
                        .await?,
                        joined: optional_parse(&member, "joined_date")?,
                        left: optional_parse(&member, "left_date")?,
                    });
                }
                ArtistKind::Group { members }
            }
            "Unknown" => ArtistKind::Unknown,
            _ => return Err(Error::msg("Invalid artist kind")),
        };
        let font = match (
            row.try_get("theme_font_family")?,
            row.try_get("theme_font_url")?,
        ) {
            (Some(family), Some(url)) => Some(Box::new(Font { family, url })),
            (None, None) => None,
            _ => return Err(Error::msg("An artist font needs both a family and a URL")),
        };
        Ok(Self {
            id: parse(&row, "id")?,
            name: row.try_get("name")?,
            sort_name: row.try_get("sort_name")?,
            kind,
            biography: row.try_get("biography")?,
            begin: optional_parse(&row, "begin_date")?,
            end: optional_parse(&row, "end_date")?,
            origin: row.try_get("origin")?,
            tags: read_tags(db, row.try_get("id")?).await?,
            artwork_square: artwork(db, row.try_get("id")?, ArtistArtworkKind::Square).await?,
            artwork_banner: artwork(db, row.try_get("id")?, ArtistArtworkKind::Banner).await?,
            artwork_portrait: artwork(db, row.try_get("id")?, ArtistArtworkKind::Portrait).await?,
            artwork_motion_banner: artwork(db, row.try_get("id")?, ArtistArtworkKind::MotionBanner)
                .await?,
            theme: theme(&row)?,
            font,
            external_ids: external_ids(db, row.try_get("id")?).await?,
        })
    }

    async fn select_relations(
        &mut self,
        db: &mut SqliteConnection,
        options: LookupOptions,
    ) -> Result<()> {
        if options.visuals == VisualData::None {
            self.artwork_square = None;
        }
        if options.visuals == VisualData::None
            || options.visuals == VisualData::Primary && self.artwork_square.is_some()
        {
            self.artwork_portrait = None;
        }
        if options.visuals != VisualData::Full {
            self.theme = None;
            self.font = None;
            self.artwork_banner = None;
            self.artwork_motion_banner = None;
        }
        select_tags(db, &mut self.tags, options.tags).await
    }
}

impl DatabaseRecord for Album {
    const HAS_CREDITS: bool = true;
    const HAS_EXTERNAL_IDS: bool = true;

    async fn read(db: &mut SqliteConnection, row: SqliteRow) -> Result<Self> {
        let owner = Key::Text(row.try_get("id")?);
        let edition = match row.try_get::<Option<String>, _>("edition_name")? {
            Some(name) => Some(Edition {
                name,
                of: optional_parse(&row, "edition_of")?,
            }),
            None => None,
        };
        let album_type = match (
            row.try_get::<Option<&str>, _>("album_type")?,
            row.try_get("soundtrack_source")?,
        ) {
            (Some("Soundtrack"), source) => Some(AlbumType::Soundtrack { source }),
            (_, Some(_)) => return Err(Error::msg("Only soundtracks can have a source title")),
            (Some(name), None) => Some(unit_enum(name)?),
            (None, None) => None,
        };
        let mut discs = Vec::new();
        for disc in children(db, "album_discs", owner).await? {
            discs.push(Disc {
                title: disc.try_get("title")?,
                tracks: ordered(db, "disc_tracks", Key::Number(disc.try_get("id")?), |row| {
                    Ok(RecordRef::Id(parse(row, "track_id")?))
                })
                .await?,
            });
        }
        Ok(Self {
            id: parse(&row, "id")?,
            title: row.try_get("title")?,
            sort_title: row.try_get("sort_title")?,
            edition,
            album_type,
            release_date: optional_parse(&row, "release_date")?,
            discs,
            credits: ordered(db, "credits", owner, credit).await?,
            label: row.try_get("label")?,
            copyright: row.try_get("copyright")?,
            description: row.try_get("description")?,
            tags: read_tags(db, row.try_get("id")?).await?,
            artwork_square: artwork(db, row.try_get("id")?, AlbumArtworkKind::Square).await?,
            artwork_banner: artwork(db, row.try_get("id")?, AlbumArtworkKind::Banner).await?,
            artwork_motion_square: artwork(db, row.try_get("id")?, AlbumArtworkKind::MotionSquare)
                .await?,
            theme: theme(&row)?,
            external_ids: external_ids(db, row.try_get("id")?).await?,
        })
    }

    async fn select_relations(
        &mut self,
        db: &mut SqliteConnection,
        options: LookupOptions,
    ) -> Result<()> {
        album_visuals(self, options.visuals);
        select_tags(db, &mut self.tags, options.tags).await?;
        match options.tracks {
            RelatedData::None => self.discs.clear(),
            RelatedData::Ids => {}
            RelatedData::Full => {
                for entry in self.discs.iter_mut().flat_map(|disc| &mut disc.tracks) {
                    if let RecordRef::Id(id) = entry {
                        let mut track: Track = read_by_id(db, &id.to_string())
                            .await?
                            .ok_or_else(|| Error::msg("Album references a missing track"))?;
                        if options.visuals == VisualData::None {
                            track.artwork_canvas = None;
                        }
                        *entry = RecordRef::Data(Box::new(track));
                    }
                }
            }
        }
        Ok(())
    }
}

impl DatabaseRecord for Track {
    const COLUMNS: &'static str = "record.*, COALESCE(record.release_date,
        (SELECT release_date FROM albums WHERE id = record.album_id)) AS effective_release_date";
    const HAS_CREDITS: bool = true;
    const HAS_EXTERNAL_IDS: bool = true;

    async fn read(db: &mut SqliteConnection, row: SqliteRow) -> Result<Self> {
        let owner = Key::Text(row.try_get("id")?);
        let mut derived_from = HashMap::<Id<Self>, Vec<DerivationKind>>::new();
        for row in children(db, "track_derivations", owner).await? {
            derived_from
                .entry(parse(&row, "track_id")?)
                .or_default()
                .push(derivation_kind(&row)?);
        }
        Ok(Self {
            id: parse(&row, "id")?,
            title: row.try_get("title")?,
            duration_ms: row.try_get("duration_ms")?,
            version: row.try_get("version")?,
            credits: ordered(db, "credits", owner, credit).await?,
            album: optional_parse(&row, "album_id")?.map_or(RecordRef::None, RecordRef::Id),
            release_date: optional_parse(&row, "effective_release_date")?,
            languages: ordered(db, "track_languages", owner, |row| {
                Ok(row.try_get("language")?)
            })
            .await?,
            lyrics: HashMap::new(),
            rating: optional_enum(&row, "rating")?,
            tags: read_tags(db, row.try_get("id")?).await?,
            features: AudioFeatures {
                tempo_bpm: row.try_get("tempo_bpm")?,
                key: optional_enum(&row, "key")?,
                mode: optional_enum(&row, "mode")?,
                time_signature: row.try_get("time_signature")?,
                loudness_lufs: row.try_get("loudness_lufs")?,
                peak: row.try_get("peak")?,
                acousticness: row.try_get("acousticness")?,
                danceability: row.try_get("danceability")?,
                energy: row.try_get("energy")?,
                instrumentalness: row.try_get("instrumentalness")?,
                liveness: row.try_get("liveness")?,
                speechiness: row.try_get("speechiness")?,
                valence: row.try_get("valence")?,
            },
            about: row.try_get("about")?,
            derived_from,
            music_video: row.try_get("music_video")?,
            artwork_canvas: artwork(db, row.try_get("id")?, TrackArtworkKind::Canvas).await?,
            external_ids: external_ids(db, row.try_get("id")?).await?,
        })
    }

    async fn select_relations(
        &mut self,
        db: &mut SqliteConnection,
        options: LookupOptions,
    ) -> Result<()> {
        match options.album {
            RelatedData::None => self.album = RecordRef::None,
            RelatedData::Ids => {}
            RelatedData::Full => {
                if let RecordRef::Id(id) = &self.album {
                    let mut album: Album = read_by_id(db, &id.to_string())
                        .await?
                        .ok_or_else(|| Error::msg("Track references a missing album"))?;
                    album_visuals(&mut album, options.visuals);
                    self.album = RecordRef::Data(Box::new(album));
                }
            }
        }
        if options.visuals == VisualData::None {
            self.artwork_canvas = None;
        }
        select_tags(db, &mut self.tags, options.tags).await
    }

    async fn load_lyrics(&mut self, db: &mut SqliteConnection) -> Result<()> {
        let track_id = self.id.to_string();
        let rows = query("SELECT language, instrumental FROM track_lyrics WHERE track_id = ?")
            .bind(&track_id)
            .fetch_all(&mut *db)
            .await?;
        for row in rows {
            let language: String = row.try_get("language")?;
            let mut channels = Vec::new();
            for channel in query("SELECT id, artist_id, kind FROM lyrics_channels WHERE track_id = ? AND language = ? ORDER BY position")
                .bind(&track_id).bind(&language).fetch_all(&mut *db).await?
            {
                channels.push(Channel {
                    artist_id: optional_parse(&channel, "artist_id")?,
                    kind: unit_enum(channel.try_get("kind")?)?,
                    spans: ordered(db, "channel_spans", Key::Number(channel.try_get("id")?), |row| {
                        Ok(Span { text: row.try_get("text")?, start_ms: row.try_get("start_ms")?, end_ms: row.try_get("end_ms")? })
                    }).await?,
                });
            }
            self.lyrics.insert(
                language,
                Lyrics {
                    instrumental: row.try_get("instrumental")?,
                    channels,
                },
            );
        }
        Ok(())
    }

    fn omit_lyrics(&mut self) {
        self.lyrics.clear();
    }
}

impl DatabaseRecord for Tag {
    fn read(_db: &mut SqliteConnection, row: SqliteRow) -> impl Future<Output = Result<Self>> {
        std::future::ready((|| {
            let id = optional_parse(&row, "related_id")?;
            let relation = match (row.try_get::<Option<&str>, _>("relation_kind")?, id) {
                (Some("Parent"), Some(id)) => TagRelation::Parent(id),
                (Some("AliasOf"), Some(id)) => TagRelation::AliasOf(id),
                (None, None) => TagRelation::None,
                _ => return Err(Error::msg("Invalid tag relationship")),
            };
            Ok(Self {
                id: parse(&row, "id")?,
                name: row.try_get("name")?,
                kind: unit_enum(row.try_get("kind")?)?,
                relation,
            })
        })())
    }
}

fn credit(row: &SqliteRow) -> Result<Credit> {
    Ok(Credit {
        artist_id: parse(row, "artist_id")?,
        role: role(row)?,
    })
}

fn role(row: &SqliteRow) -> Result<Role> {
    match (row.try_get::<&str, _>("role")?, row.try_get("role_detail")?) {
        ("Instrument", Some(detail)) => Ok(Role::Instrument(detail)),
        ("Other", Some(detail)) => Ok(Role::Other(detail)),
        ("Instrument" | "Other", None) | (_, Some(_)) => {
            Err(Error::msg("Invalid credit role detail"))
        }
        (name, None) => unit_enum(name),
    }
}

async fn read_tags(db: &mut SqliteConnection, owner: &str) -> Result<Vec<Scored<RecordRef<Tag>>>> {
    query("SELECT tag_id, weight FROM record_tags WHERE owner_id = ? ORDER BY weight DESC, tag_id")
        .bind(owner)
        .fetch_all(db)
        .await?
        .iter()
        .map(weighted_tag)
        .collect()
}

fn weighted_tag(row: &SqliteRow) -> Result<Scored<RecordRef<Tag>>> {
    Ok(Scored {
        id: RecordRef::Id(parse(row, "tag_id")?),
        weight: row.try_get("weight")?,
    })
}

async fn select_tags(
    db: &mut SqliteConnection,
    tags: &mut Vec<Scored<RecordRef<Tag>>>,
    mode: RelatedData,
) -> Result<()> {
    match mode {
        RelatedData::None => tags.clear(),
        RelatedData::Ids => {}
        RelatedData::Full => {
            for tag in tags {
                if let RecordRef::Id(id) = &tag.id {
                    let record = read_by_id(db, &id.to_string())
                        .await?
                        .ok_or_else(|| Error::msg("Record references a missing tag"))?;
                    tag.id = RecordRef::Data(Box::new(record));
                }
            }
        }
    }
    Ok(())
}

fn derivation_kind(row: &SqliteRow) -> Result<DerivationKind> {
    let range = |start, end| -> Result<Option<TimeRange>> {
        Ok(match row.try_get(start)? {
            Some(start_ms) => Some(TimeRange {
                start_ms,
                end_ms: row.try_get(end)?,
            }),
            None => None,
        })
    };
    let from_range = range("from_start_ms", "from_end_ms")?;
    let to_range = range("to_start_ms", "to_end_ms")?;
    let kind = match row.try_get::<&str, _>("kind")? {
        "Sample" => DerivationKind::Sample {
            from_range,
            to_range,
        },
        name if from_range.is_none() && to_range.is_none() => unit_enum(name)?,
        _ => return Err(Error::msg("Only sample derivations can have time ranges")),
    };
    Ok(kind)
}

async fn external_ids(db: &mut SqliteConnection, owner: &str) -> Result<ExternalIds> {
    let mut ids = ExternalIds::new();
    for row in query(
        "SELECT provider, value FROM external_ids WHERE owner_id = ? ORDER BY provider, value",
    )
    .bind(owner)
    .fetch_all(db)
    .await?
    {
        let label: &str = row.try_get("provider")?;
        let provider = match label.strip_prefix("other:") {
            Some(name) => ExternalIdProvider::Other(name.to_owned()),
            None => unit_enum(label)?,
        };
        ids.entry(provider).or_default().push(row.try_get("value")?);
    }
    Ok(ids)
}

fn album_visuals(album: &mut Album, mode: VisualData) {
    if mode == VisualData::None {
        album.artwork_square = None;
    }
    if mode != VisualData::Full {
        album.theme = None;
        album.artwork_banner = None;
        album.artwork_motion_square = None;
    }
}

async fn artwork(
    db: &mut SqliteConnection,
    record: &str,
    slot: impl std::fmt::Debug + Send,
) -> Result<Option<Box<Artwork>>> {
    let Some(art) =
        query("SELECT id, blurhash, explicit FROM artwork WHERE owner_id = ? AND kind = ?")
            .bind(record)
            .bind(format!("{slot:?}"))
            .fetch_optional(&mut *db)
            .await?
    else {
        return Ok(None);
    };
    let owner = Key::Number(art.try_get("id")?);
    Ok(Some(Box::new(Artwork {
        files: ordered(db, "artwork_renditions", owner, |row| {
            Ok(Rendition {
                url: row.try_get("url")?,
                width: row.try_get("width")?,
                height: row.try_get("height")?,
                codec: row.try_get("codec")?,
                duration_ms: row.try_get("duration_ms")?,
            })
        })
        .await?,
        colors: ordered(db, "artwork_colors", owner, |row| parse(row, "color")).await?,
        blurhash: art.try_get("blurhash")?,
        explicit: art.try_get("explicit")?,
    })))
}

fn theme(row: &SqliteRow) -> Result<Option<Scheme>> {
    match (
        optional_parse(row, "theme_background")?,
        optional_parse(row, "theme_foreground")?,
        optional_parse(row, "theme_accent")?,
    ) {
        (Some(background), Some(foreground), Some(accent)) => Ok(Some(Scheme {
            background,
            foreground,
            accent,
        })),
        (None, None, None) => Ok(None),
        _ => Err(Error::msg(
            "A theme scheme needs background, foreground, and accent colors",
        )),
    }
}

#[derive(Clone, Copy)]
enum Key<'a> {
    Text(&'a str),
    Number(i64),
}

async fn children(
    db: &mut SqliteConnection,
    table: &'static str,
    owner: Key<'_>,
) -> Result<Vec<SqliteRow>> {
    // All table names are literals at the call sites. Owner IDs are bound parameters.
    let sql = format!("SELECT * FROM {table} WHERE owner_id = ? ORDER BY position");
    let statement = query::<Sqlite>(AssertSqlSafe(sql));
    let statement = match owner {
        Key::Text(id) => statement.bind(id),
        Key::Number(id) => statement.bind(id),
    };
    Ok(statement.fetch_all(db).await?)
}

async fn ordered<T>(
    db: &mut SqliteConnection,
    table: &'static str,
    owner: Key<'_>,
    convert: impl Fn(&SqliteRow) -> Result<T> + Send,
) -> Result<Vec<T>> {
    children(db, table, owner)
        .await?
        .iter()
        .map(convert)
        .collect()
}

fn parse<T: FromStr>(row: &SqliteRow, column: &str) -> Result<T> {
    row.try_get::<&str, _>(column)?
        .parse()
        .map_err(|_| Error::msg(format!("Invalid value in catalog column {column}")))
}

fn optional_parse<T: FromStr>(row: &SqliteRow, column: &str) -> Result<Option<T>> {
    row.try_get::<Option<&str>, _>(column)?
        .map(|value| {
            value
                .parse()
                .map_err(|_| Error::msg(format!("Invalid value in catalog column {column}")))
        })
        .transpose()
}

fn unit_enum<T: DeserializeOwned>(name: &str) -> Result<T> {
    Ok(T::deserialize(
        StrDeserializer::<serde::de::value::Error>::new(name),
    )?)
}

fn optional_enum<T: DeserializeOwned>(row: &SqliteRow, column: &str) -> Result<Option<T>> {
    row.try_get::<Option<&str>, _>(column)?
        .map(unit_enum)
        .transpose()
}
