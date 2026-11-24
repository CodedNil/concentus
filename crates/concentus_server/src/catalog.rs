use crate::{
    records::{DatabaseRecord, read_by_id},
    store::Store,
};
use concentus_types::{Album, Artist, LookupOptions, RelatedData, Track};
use sqlx::{AssertSqlSafe, QueryBuilder, Sqlite, query_as};
use topcoat::Result;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Artist,
    Album,
    Track,
    Tag,
}

impl Kind {
    pub const MUSIC: [Self; 3] = [Self::Track, Self::Album, Self::Artist];
    pub const ALL: [Self; 4] = [Self::Track, Self::Album, Self::Artist, Self::Tag];

    pub fn parse(value: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|kind| kind.path() == value)
    }

    pub const fn path(self) -> &'static str {
        match self {
            Self::Artist => "artists",
            Self::Album => "albums",
            Self::Track => "tracks",
            Self::Tag => "tags",
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::Artist => "Artist",
            Self::Album => "Album",
            Self::Track => "Track",
            Self::Tag => "Tag",
        }
    }

    pub const fn plural(self) -> &'static str {
        match self {
            Self::Artist => "Artists",
            Self::Album => "Albums",
            Self::Track => "Tracks",
            Self::Tag => "Tags",
        }
    }

    pub const fn title_column(self) -> &'static str {
        match self {
            Self::Artist | Self::Tag => "name",
            Self::Album | Self::Track => "title",
        }
    }

    const fn has_credits(self) -> bool {
        matches!(self, Self::Album | Self::Track)
    }

    pub fn href(self, id: impl std::fmt::Display) -> String {
        format!("/catalog/{}/{id}", self.path())
    }
}

#[derive(Debug, sqlx::FromRow)]
pub struct Entry {
    pub id: String,
    pub title: String,
    pub subtitle: Option<String>,
    pub preview: Option<String>,
    pub image: Option<String>,
    pub release_id: Option<String>,
    pub release_group_id: Option<String>,
}

pub enum Detail {
    Artist(Artist),
    Album(Album),
    Track(Box<Track>),
}

impl Store {
    pub async fn popular_entries(&self, kind: Kind, after: &str, limit: u32) -> Result<Vec<Entry>> {
        let columns = entry_columns(kind);
        let sql = format!(
            "WITH popularity AS MATERIALIZED (
                SELECT record_id, sum(count) AS hits
                FROM record_access_daily WHERE day >= unixepoch() / 86400 - 6
                GROUP BY record_id
            )
            SELECT {columns} FROM {} AS item
            LEFT JOIN popularity ON popularity.record_id = item.id
            WHERE ?1 = '' OR (-coalesce(popularity.hits, 0), item.id) >
                (-coalesce((SELECT hits FROM popularity WHERE record_id = ?1), 0), ?1)
            ORDER BY coalesce(popularity.hits, 0) DESC, item.id LIMIT ?2",
            kind.path()
        );
        Ok(query_as::<_, Entry>(AssertSqlSafe(sql))
            .bind(after)
            .bind(limit)
            .fetch_all(&self.0)
            .await?)
    }

    pub async fn recent_entries(&self, kind: Kind, limit: u32) -> Result<Vec<Entry>> {
        let columns = entry_columns(kind);
        let visibility = if kind == Kind::Tag {
            "WHERE item.kind != 'Hidden'"
        } else {
            ""
        };
        let sql = format!(
            "SELECT {columns} FROM {} AS item {visibility} ORDER BY item.id DESC LIMIT ?",
            kind.path()
        );
        Ok(query_as::<_, Entry>(AssertSqlSafe(sql))
            .bind(limit)
            .fetch_all(&self.0)
            .await?)
    }

    pub async fn entries(
        &self,
        kind: Kind,
        input: &str,
        after: &str,
        parent: Option<(Kind, &str)>,
        limit: u32,
    ) -> Result<Vec<Entry>> {
        let title = kind.title_column();
        let columns = entry_columns(kind);
        let related = match parent {
            Some((Kind::Artist, _)) if kind == Kind::Artist => {
                "EXISTS (SELECT 1 FROM artist_members WHERE owner_id = item.id AND artist_id = ?4)"
            }
            Some((Kind::Artist, _)) if kind.has_credits() => {
                "EXISTS (SELECT 1 FROM credits WHERE owner_id = item.id AND artist_id = ?4)"
            }
            Some((Kind::Track, _)) if kind == Kind::Album => {
                "EXISTS (SELECT 1 FROM album_discs AS disc JOIN disc_tracks AS track ON track.owner_id = disc.id WHERE disc.owner_id = item.id AND track.track_id = ?4)"
            }
            Some((Kind::Tag, _)) if kind == Kind::Tag => "item.related_id = ?4",
            Some((Kind::Tag, _)) => {
                "EXISTS (SELECT 1 FROM record_tags WHERE owner_id = item.id AND tag_id = ?4)"
            }
            Some(_) => "0",
            None => "1",
        };
        let parent_id = parent.map(|(_, id)| id);
        let (artist, query) = input
            .split_once(" — ")
            .or_else(|| input.split_once(" - "))
            .filter(|(artist, title)| {
                kind.has_credits() && !artist.trim().is_empty() && !title.trim().is_empty()
            })
            .map_or(("", input), |(artist, title)| (artist.trim(), title.trim()));
        let additional = match kind {
            Kind::Tag => {
                "OR EXISTS (SELECT 1 FROM tags AS alias WHERE alias.related_id = item.id AND alias.relation_kind = 'AliasOf' AND instr(lower(alias.name), lower(?1)) > 0)"
            }
            Kind::Artist => "OR instr(lower(coalesce(item.sort_name, '')), lower(?1)) > 0",
            _ => {
                "OR (?2 = '' AND (
                instr(lower(coalesce(subtitle, '')), lower(?1)) > 0
                OR instr(lower(coalesce(subtitle, '') || ' ' || title), lower(?1)) > 0
                OR instr(lower(title || ' ' || coalesce(subtitle, '')), lower(?1)) > 0))"
            }
        };
        let external = if kind == Kind::Tag {
            ""
        } else {
            "OR EXISTS (SELECT 1 FROM external_ids WHERE owner_id = item.id AND value = ?1 COLLATE NOCASE)"
        };
        let visibility = if kind == Kind::Tag {
            "AND item.kind != 'Hidden'"
        } else {
            ""
        };
        let fts = search_terms(query);
        let matches = if fts.is_empty() {
            ""
        } else {
            "OR item.id IN (SELECT record_id FROM catalog_search WHERE catalog_search MATCH ?6 AND collection = ?7)"
        };
        let sql = format!(
            "SELECT {columns} FROM {} AS item
            WHERE (?1 = '' OR item.id = ?1 OR instr(lower(item.{title}), lower(?1)) > 0 {additional} {external} {matches})
            AND (?2 = '' OR instr(lower(coalesce(subtitle, '')), lower(?2)) > 0)
            AND item.id > ?3 AND ({related}) {visibility}
            ORDER BY item.id LIMIT ?5",
            kind.path()
        );
        let statement = query_as::<_, Entry>(AssertSqlSafe(sql))
            .bind(query)
            .bind(artist)
            .bind(after)
            .bind(parent_id)
            .bind(limit);
        let statement = if fts.is_empty() {
            statement
        } else {
            statement.bind(fts).bind(kind.path())
        };
        Ok(statement.fetch_all(&self.0).await?)
    }

    pub async fn entries_by_id(&self, kind: Kind, ids: &[String]) -> Result<Vec<Entry>> {
        let sql = format!(
            "SELECT {} FROM {} AS item WHERE item.id IN (",
            entry_columns(kind),
            kind.path()
        );
        let mut entries = Vec::new();
        for chunk in ids.chunks(200) {
            let mut query = QueryBuilder::<Sqlite>::new(sql.as_str());
            {
                let mut list = query.separated(", ");
                for id in chunk {
                    list.push_bind(id);
                }
            }
            query.push(")");
            entries.extend(query.build_query_as::<Entry>().fetch_all(&self.0).await?);
        }
        Ok(entries)
    }

    pub async fn detail(&self, kind: Kind, id: &str) -> Result<Option<Detail>> {
        let mut db = self.0.begin().await?;
        let detail = match kind {
            Kind::Artist => view_record(&mut db, id).await?.map(Detail::Artist),
            Kind::Album => view_record(&mut db, id).await?.map(Detail::Album),
            Kind::Track => view_record(&mut db, id)
                .await?
                .map(Box::new)
                .map(Detail::Track),
            Kind::Tag => None,
        };
        db.commit().await?;
        Ok(detail)
    }
}

fn entry_columns(kind: Kind) -> String {
    let title = kind.title_column();
    let subtitle = subtitle_sql(kind);
    let preview = preview_sql(kind);
    let album = match kind {
        Kind::Album => "item.id",
        Kind::Track => "item.album_id",
        _ => "NULL",
    };
    let image = match kind {
        Kind::Artist => format!(
            "coalesce({}, {})",
            image_sql("item.id", "Square"),
            image_sql("item.id", "Portrait")
        ),
        Kind::Album | Kind::Track => image_sql(album, "Square"),
        Kind::Tag => "NULL".into(),
    };
    let group = crate::enrichment::cover_art_archive::RELEASE_GROUP_PROVIDER;
    format!("item.id, item.{title} AS title, {subtitle} AS subtitle, {preview} AS preview, {image} AS image,
        (SELECT value FROM external_ids WHERE owner_id = {album} AND provider = 'Musicbrainz' ORDER BY value LIMIT 1) AS release_id,
        (SELECT value FROM external_ids WHERE owner_id = {album} AND provider = 'other:{group}' ORDER BY value LIMIT 1) AS release_group_id")
}

fn image_sql(owner: &str, kind: &str) -> String {
    format!(
        "(SELECT rendition.url FROM artwork AS art
        JOIN artwork_renditions AS rendition ON rendition.owner_id = art.id
        WHERE art.owner_id = {owner} AND art.kind = '{kind}' AND rendition.duration_ms IS NULL
        ORDER BY art.explicit, rendition.position LIMIT 1)"
    )
}

async fn view_record<T: DatabaseRecord>(
    db: &mut sqlx::SqliteConnection,
    id: &str,
) -> Result<Option<T>> {
    if let Some(mut record) = read_by_id::<T>(db, id).await? {
        record.load_lyrics(db).await?;
        record
            .select_relations(
                db,
                LookupOptions {
                    tags: RelatedData::Full,
                    ..LookupOptions::default()
                },
            )
            .await?;
        Ok(Some(record))
    } else {
        Ok(None)
    }
}

fn subtitle_sql(kind: Kind) -> &'static str {
    if kind.has_credits() {
        return "(SELECT group_concat(name, ', ') FROM (
            SELECT artist.name FROM credits AS credit
            JOIN artists AS artist ON artist.id = credit.artist_id
            WHERE credit.owner_id = item.id AND credit.role IN ('Primary', 'Featured')
            ORDER BY credit.position))";
    }
    match kind {
        Kind::Artist => "coalesce(item.origin, item.kind)",
        Kind::Tag => "item.kind",
        _ => unreachable!(),
    }
}

const fn preview_sql(kind: Kind) -> &'static str {
    match kind {
        Kind::Artist => "nullif(concat_ws(' · ', item.kind, item.origin, item.begin_date), '')",
        Kind::Album => "nullif(concat_ws(' · ', item.album_type, item.release_date, item.label), '')",
        Kind::Track => "nullif(concat_ws(' · ', item.version, item.release_date,
            CASE WHEN item.duration_ms > 0 THEN printf('%d:%02d', item.duration_ms / 60000, item.duration_ms / 1000 % 60) END), '')",
        Kind::Tag => "nullif(concat_ws(' · ', item.kind,
            (SELECT group_concat(alias.name, ', ') FROM tags AS alias WHERE alias.related_id = item.id AND alias.relation_kind = 'AliasOf')), '')",
    }
}

fn search_terms(query: &str) -> String {
    query
        .split(|character: char| !character.is_alphanumeric())
        .filter(|word| !word.is_empty())
        .map(|word| format!("\"{word}\"*"))
        .collect::<Vec<_>>()
        .join(" AND ")
}
