//! Maps normalized SQLite rows to the shared metadata types.
use std::{collections::HashMap, future::Future, str::FromStr};

use concentus_types::{
    Album, AlbumKind, Artist, ArtistKind, Artwork, AudioFeatures, Channel, Collection, Credit,
    Derivation, DerivationKind, Disc, Edition, ExternalId, Font, Lyrics, Membership, Rendition,
    Role, Scheme, Span, Tag, Theme, TimeRange, Track, Visuals, Weighted,
};
use serde::de::{DeserializeOwned, value::StrDeserializer};
use sqlx::{AssertSqlSafe, Row, Sqlite, SqliteConnection, query, sqlite::SqliteRow};
use topcoat::{Error, Result};

pub trait DatabaseRecord: Collection + Send {
    const CREDIT_TABLE: Option<&'static str> = None;
    const EXTERNAL_ID_TABLE: Option<&'static str> = None;

    fn read(db: &mut SqliteConnection, row: SqliteRow)
    -> impl Future<Output = Result<Self>> + Send;

    fn load_lyrics(
        &mut self,
        _db: &mut SqliteConnection,
    ) -> impl Future<Output = Result<()>> + Send {
        async { Ok(()) }
    }

    fn omit_lyrics(&mut self) {}
}

impl DatabaseRecord for Artist {
    const EXTERNAL_ID_TABLE: Option<&'static str> = Some("artist_external_ids");

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
                    });
                }
                ArtistKind::Group { members }
            }
            "Unknown" => ArtistKind::Unknown,
            _ => return Err(Error::msg("Invalid artist kind")),
        };
        Ok(Self {
            id: parse(&row, "id")?,
            name: row.try_get("name")?,
            sort_name: row.try_get("sort_name")?,
            kind,
            biography: row.try_get("biography")?,
            begin: optional_parse(&row, "begin")?,
            end: optional_parse(&row, "end")?,
            origin: row.try_get("origin")?,
            tags: ordered(db, "artist_tags", owner, weighted_tag).await?,
            visuals: visuals(db, row.try_get("visuals_id")?).await?,
            external_ids: ordered(db, "artist_external_ids", owner, external_id).await?,
        })
    }
}

impl DatabaseRecord for Album {
    const CREDIT_TABLE: Option<&'static str> = Some("album_credits");
    const EXTERNAL_ID_TABLE: Option<&'static str> = Some("album_external_ids");

    async fn read(db: &mut SqliteConnection, row: SqliteRow) -> Result<Self> {
        let owner = Key::Text(row.try_get("id")?);
        let edition = match row.try_get::<Option<String>, _>("edition_name")? {
            Some(name) => Some(Edition {
                name,
                of: optional_parse(&row, "edition_of")?,
            }),
            None => None,
        };
        let kind = match row.try_get::<&str, _>("kind")? {
            "Soundtrack" => AlbumKind::Soundtrack {
                source: row.try_get("soundtrack_source")?,
            },
            kind => unit_enum(kind)?,
        };
        let mut discs = Vec::new();
        for disc in children(db, "album_discs", owner).await? {
            discs.push(Disc {
                title: disc.try_get("title")?,
                track_ids: ordered(db, "disc_tracks", Key::Number(disc.try_get("id")?), |row| {
                    parse(row, "track_id")
                })
                .await?,
            });
        }
        Ok(Self {
            id: parse(&row, "id")?,
            title: row.try_get("title")?,
            sort_title: row.try_get("sort_title")?,
            edition,
            kind,
            release_date: optional_parse(&row, "release_date")?,
            discs,
            credits: ordered(db, "album_credits", owner, credit).await?,
            label: row.try_get("label")?,
            copyright: row.try_get("copyright")?,
            description: row.try_get("description")?,
            tags: ordered(db, "album_tags", owner, weighted_tag).await?,
            visuals: visuals(db, row.try_get("visuals_id")?).await?,
            external_ids: ordered(db, "album_external_ids", owner, external_id).await?,
        })
    }
}

impl DatabaseRecord for Track {
    const CREDIT_TABLE: Option<&'static str> = Some("track_credits");
    const EXTERNAL_ID_TABLE: Option<&'static str> = Some("track_external_ids");

    async fn read(db: &mut SqliteConnection, row: SqliteRow) -> Result<Self> {
        let owner = Key::Text(row.try_get("id")?);
        Ok(Self {
            id: parse(&row, "id")?,
            title: row.try_get("title")?,
            duration_ms: row.try_get("duration_ms")?,
            version: row.try_get("version")?,
            credits: ordered(db, "track_credits", owner, credit).await?,
            album_id: optional_parse(&row, "album_id")?,
            languages: ordered(db, "track_languages", owner, |row| {
                Ok(row.try_get("language")?)
            })
            .await?,
            lyrics: HashMap::new(),
            rating: optional_enum(&row, "rating")?,
            tags: ordered(db, "track_tags", owner, weighted_tag).await?,
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
            derived_from: ordered(db, "track_derivations", owner, derivation).await?,
            music_video: optional_parse(&row, "music_video")?,
            visuals: visuals(db, row.try_get("visuals_id")?).await?,
            external_ids: ordered(db, "track_external_ids", owner, external_id).await?,
        })
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
    async fn read(db: &mut SqliteConnection, row: SqliteRow) -> Result<Self> {
        Ok(Self {
            id: parse(&row, "id")?,
            name: row.try_get("name")?,
            kind: unit_enum(row.try_get("kind")?)?,
            aliases: ordered(db, "tag_aliases", Key::Text(row.try_get("id")?), |row| {
                Ok(row.try_get("alias")?)
            })
            .await?,
            parent: optional_parse(&row, "parent")?,
        })
    }
}

fn credit(row: &SqliteRow) -> Result<Credit> {
    Ok(Credit {
        artist_id: parse(row, "artist_id")?,
        role: role(row)?,
    })
}

fn role(row: &SqliteRow) -> Result<Role> {
    match row.try_get::<&str, _>("role")? {
        "Instrument" => Ok(Role::Instrument(row.try_get("role_detail")?)),
        "Other" => Ok(Role::Other(row.try_get("role_detail")?)),
        name => unit_enum(name),
    }
}

fn weighted_tag(row: &SqliteRow) -> Result<Weighted<Tag>> {
    Ok(Weighted {
        id: parse(row, "tag_id")?,
        weight: row.try_get("weight")?,
    })
}

fn derivation(row: &SqliteRow) -> Result<Derivation> {
    let range = |start, end| -> Result<Option<TimeRange>> {
        Ok(match row.try_get(start)? {
            Some(start_ms) => Some(TimeRange {
                start_ms,
                end_ms: row.try_get(end)?,
            }),
            None => None,
        })
    };
    let kind = match row.try_get::<&str, _>("kind")? {
        "Sample" => DerivationKind::Sample {
            from_range: range("from_start_ms", "from_end_ms")?,
            to_range: range("to_start_ms", "to_end_ms")?,
        },
        name => unit_enum(name)?,
    };
    Ok(Derivation {
        track_id: parse(row, "track_id")?,
        kind,
    })
}

fn external_id(row: &SqliteRow) -> Result<ExternalId> {
    Ok(match row.try_get::<&str, _>("provider")? {
        "Musicbrainz" => ExternalId::Musicbrainz(parse(row, "value_text")?),
        "Isrc" => ExternalId::Isrc(parse(row, "value_text")?),
        "Barcode" => ExternalId::Barcode(parse(row, "value_number")?),
        "Wikidata" => ExternalId::Wikidata(parse(row, "value_number")?),
        "Discogs" => ExternalId::Discogs(parse(row, "value_number")?),
        "Genius" => ExternalId::Genius(parse(row, "value_number")?),
        "AppleMusic" => ExternalId::AppleMusic(parse(row, "value_number")?),
        "Deezer" => ExternalId::Deezer(parse(row, "value_number")?),
        "Tidal" => ExternalId::Tidal(parse(row, "value_number")?),
        "Spotify" => ExternalId::Spotify(parse(row, "value_text")?),
        "Amazon" => ExternalId::Amazon(parse(row, "value_text")?),
        "YoutubeMusic" => ExternalId::YoutubeMusic(row.try_get("value_text")?),
        "Isni" => ExternalId::Isni(parse(row, "value_text")?),
        "Ipi" => ExternalId::Ipi(parse(row, "value_number")?),
        "Other" => ExternalId::Other {
            name: row.try_get("other_name")?,
            id: row.try_get("value_text")?,
        },
        _ => return Err(Error::msg("Invalid external ID provider")),
    })
}

async fn visuals(db: &mut SqliteConnection, id: Option<i64>) -> Result<Visuals> {
    let Some(id) = id else {
        return Ok(Visuals::default());
    };
    let row = query("SELECT light_scheme_id, dark_scheme_id, font_id FROM visuals WHERE id = ?")
        .bind(id)
        .fetch_one(&mut *db)
        .await?;
    let mut artwork = Vec::new();
    for art in children(db, "artwork", Key::Number(id)).await? {
        let owner = Key::Number(art.try_get("id")?);
        artwork.push(Artwork {
            kind: unit_enum(art.try_get("kind")?)?,
            files: ordered(db, "artwork_renditions", owner, |row| {
                Ok(Rendition {
                    url: parse(row, "url")?,
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
        });
    }
    let font = match row.try_get::<Option<i64>, _>("font_id")? {
        Some(font_id) => {
            let font = query("SELECT family, url FROM fonts WHERE id = ?")
                .bind(font_id)
                .fetch_one(&mut *db)
                .await?;
            Some(Font {
                family: font.try_get("family")?,
                url: parse(&font, "url")?,
            })
        }
        None => None,
    };
    Ok(Visuals {
        artwork,
        theme: Theme {
            light: scheme(db, row.try_get("light_scheme_id")?).await?,
            dark: scheme(db, row.try_get("dark_scheme_id")?).await?,
            font,
        },
    })
}

async fn scheme(db: &mut SqliteConnection, id: Option<i64>) -> Result<Option<Scheme>> {
    let Some(id) = id else {
        return Ok(None);
    };
    let row = query("SELECT background, foreground, accent FROM schemes WHERE id = ?")
        .bind(id)
        .fetch_one(db)
        .await?;
    Ok(Some(Scheme {
        background: parse(&row, "background")?,
        foreground: parse(&row, "foreground")?,
        accent: parse(&row, "accent")?,
    }))
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
