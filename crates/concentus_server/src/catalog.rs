use crate::{
    records::{DatabaseRecord, read_by_id},
    store::Store,
};
use concentus_types::{Album, Artist, LookupOptions, RelatedData, Tag, Track};
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

    pub const fn credit_table(self) -> Option<&'static str> {
        match self {
            Self::Album => Some("album_credits"),
            Self::Track => Some("track_credits"),
            _ => None,
        }
    }

    pub const fn tag_table(self) -> Option<&'static str> {
        match self {
            Self::Artist => Some("artist_tags"),
            Self::Album => Some("album_tags"),
            Self::Track => Some("track_tags"),
            Self::Tag => None,
        }
    }

    pub const fn external_id_table(self) -> Option<&'static str> {
        match self {
            Self::Artist => Some("artist_external_ids"),
            Self::Album => Some("album_external_ids"),
            Self::Track => Some("track_external_ids"),
            Self::Tag => None,
        }
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
    Tag(Tag),
}

impl Store {
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
            Some((Kind::Artist, _)) if kind == Kind::Artist => "EXISTS (SELECT 1 FROM artist_members WHERE owner_id = item.id AND artist_id = ?4)".into(),
            Some((Kind::Artist, _)) => kind.credit_table().map_or_else(|| "0".into(), |table| format!("EXISTS (SELECT 1 FROM {table} AS credit WHERE credit.owner_id = item.id AND credit.artist_id = ?4)")),
            Some((Kind::Track, _)) if kind == Kind::Album => "EXISTS (SELECT 1 FROM album_discs AS disc JOIN disc_tracks AS track ON track.owner_id = disc.id WHERE disc.owner_id = item.id AND track.track_id = ?4)".into(),
            Some((Kind::Tag, _)) if kind == Kind::Tag => "item.parent = ?4".into(),
            Some((Kind::Tag, _)) => kind.tag_table().map_or_else(|| "0".into(), |table| format!("EXISTS (SELECT 1 FROM {table} AS tag WHERE tag.owner_id = item.id AND tag.tag_id = ?4)")),
            Some(_) => "0".into(),
            None => "1".into(),
        };
        let parent_id = parent.map(|(_, id)| id);
        let (artist, query) = input
            .split_once(" — ")
            .or_else(|| input.split_once(" - "))
            .filter(|(artist, title)| {
                kind.credit_table().is_some()
                    && !artist.trim().is_empty()
                    && !title.trim().is_empty()
            })
            .map_or(("", input), |(artist, title)| (artist.trim(), title.trim()));
        let additional = match kind {
            Kind::Tag => {
                "OR EXISTS (SELECT 1 FROM tag_aliases WHERE owner_id = item.id AND instr(lower(alias), lower(?1)) > 0)"
            }
            Kind::Artist => "OR instr(lower(coalesce(item.sort_name, '')), lower(?1)) > 0",
            _ => {
                "OR (?2 = '' AND (
                instr(lower(coalesce(subtitle, '')), lower(?1)) > 0
                OR instr(lower(coalesce(subtitle, '') || ' ' || title), lower(?1)) > 0
                OR instr(lower(title || ' ' || coalesce(subtitle, '')), lower(?1)) > 0))"
            }
        };
        let external = kind.external_id_table().map_or_else(String::new, |table| {
            format!("OR EXISTS (SELECT 1 FROM {table} WHERE owner_id = item.id AND value = ?1 COLLATE NOCASE)")
        });
        let sql = format!(
            "SELECT {columns} FROM {} AS item
            WHERE (?1 = '' OR item.id = ?1 OR instr(lower(item.{title}), lower(?1)) > 0 {additional} {external})
            AND (?2 = '' OR instr(lower(coalesce(subtitle, '')), lower(?2)) > 0)
            AND item.id > ?3 AND ({related})
            ORDER BY item.id LIMIT ?5",
            kind.path()
        );
        Ok(query_as::<_, Entry>(AssertSqlSafe(sql))
            .bind(query)
            .bind(artist)
            .bind(after)
            .bind(parent_id)
            .bind(limit)
            .fetch_all(&self.0)
            .await?)
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
            Kind::Tag => view_record(&mut db, id).await?.map(Detail::Tag),
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
        Kind::Artist => image_sql("artist", "item.id", "Portrait"),
        Kind::Album => image_sql("album", album, "Cover"),
        Kind::Track => format!(
            "coalesce({}, {})",
            image_sql("track", "item.id", "Cover"),
            image_sql("album", album, "Cover")
        ),
        Kind::Tag => "NULL".into(),
    };
    let group = crate::enrichment::cover_art_archive::RELEASE_GROUP_PROVIDER;
    format!("item.id, item.{title} AS title, {subtitle} AS subtitle, {preview} AS preview, {image} AS image,
        (SELECT value FROM album_external_ids WHERE owner_id = {album} AND provider = 'Musicbrainz' ORDER BY value LIMIT 1) AS release_id,
        (SELECT value FROM album_external_ids WHERE owner_id = {album} AND provider = 'other:{group}' ORDER BY value LIMIT 1) AS release_group_id")
}

fn image_sql(table: &str, owner: &str, kind: &str) -> String {
    format!(
        "(SELECT rendition.url FROM {table}_artwork AS art
        JOIN {table}_artwork_renditions AS rendition ON rendition.owner_id = art.id
        WHERE art.owner_id = {owner} AND art.kind = '{kind}' AND rendition.duration_ms IS NULL
        ORDER BY art.explicit, art.position, rendition.position LIMIT 1)"
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

fn subtitle_sql(kind: Kind) -> String {
    if let Some(table) = kind.credit_table() {
        return format!(
            "(SELECT group_concat(name, ', ') FROM (
            SELECT artist.name FROM {table} AS credit
            JOIN artists AS artist ON artist.id = credit.artist_id
            WHERE credit.owner_id = item.id AND credit.role IN ('Primary', 'Featured')
            ORDER BY credit.position))"
        );
    }
    match kind {
        Kind::Artist => "coalesce(item.origin, item.kind)",
        Kind::Tag => "item.kind",
        _ => unreachable!(),
    }
    .into()
}

const fn preview_sql(kind: Kind) -> &'static str {
    match kind {
        Kind::Artist => "nullif(concat_ws(' · ', item.kind, item.origin, item.begin_date), '')",
        Kind::Album => "nullif(concat_ws(' · ', item.album_type, item.release_date, item.label), '')",
        Kind::Track => "nullif(concat_ws(' · ', item.version, item.release_date,
            CASE WHEN item.duration_ms > 0 THEN printf('%d:%02d', item.duration_ms / 60000, item.duration_ms / 1000 % 60) END), '')",
        Kind::Tag => "nullif(concat_ws(' · ', item.kind,
            (SELECT group_concat(alias, ', ') FROM tag_aliases WHERE owner_id = item.id)), '')",
    }
}
