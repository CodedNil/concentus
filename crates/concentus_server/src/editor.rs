use crate::{catalog::Kind, store::Store};
use serde_json::{Map, Value};
use sqlx::{AssertSqlSafe, QueryBuilder, Sqlite, SqliteConnection, query, query_scalar};
use std::collections::BTreeMap;
use topcoat::{Error, Result};

pub type Snapshot = BTreeMap<String, Value>;

#[derive(Clone, Copy)]
struct Table {
    name: &'static str,
    scope: &'static str,
    root: &'static str,
}

const fn table(name: &'static str, root: &'static str) -> Table {
    Table {
        name,
        scope: root,
        root,
    }
}

const fn children(name: &'static str, scope: &'static str) -> Table {
    Table {
        name,
        scope,
        root: "",
    }
}

const ARTWORK: &[Table] = &[
    table("artwork", "owner_id = ?1"),
    children(
        "artwork_renditions",
        "owner_id IN (SELECT id FROM artwork WHERE owner_id = ?1)",
    ),
    children(
        "artwork_colors",
        "owner_id IN (SELECT id FROM artwork WHERE owner_id = ?1)",
    ),
];

macro_rules! direct {
    ($name:literal) => {
        &[table($name, "owner_id = ?1")]
    };
}

const fn groups(kind: Kind) -> &'static [(&'static str, &'static [Table])] {
    match kind {
        Kind::Artist => {
            const {
                &[
                    ("artwork", ARTWORK),
                    (
                        "members",
                        &[
                            table("artist_members", "owner_id = ?1"),
                            children(
                                "member_roles",
                                "owner_id IN (SELECT id FROM artist_members WHERE owner_id = ?1)",
                            ),
                        ],
                    ),
                    ("tags", direct!("record_tags")),
                    ("external_ids", direct!("external_ids")),
                ]
            }
        }
        Kind::Album => {
            const {
                &[
                    ("artwork", ARTWORK),
                    ("credits", direct!("credits")),
                    (
                        "discs",
                        &[
                            table("album_discs", "owner_id = ?1"),
                            children(
                                "disc_tracks",
                                "owner_id IN (SELECT id FROM album_discs WHERE owner_id = ?1)",
                            ),
                        ],
                    ),
                    ("tags", direct!("record_tags")),
                    ("external_ids", direct!("external_ids")),
                ]
            }
        }
        Kind::Track => {
            const {
                &[
                    ("artwork", ARTWORK),
                    ("credits", direct!("credits")),
                    ("languages", direct!("track_languages")),
                    ("derived_from", direct!("track_derivations")),
                    ("tags", direct!("record_tags")),
                    ("external_ids", direct!("external_ids")),
                    (
                        "lyrics",
                        &[
                            table("track_lyrics", "track_id = ?1"),
                            table("lyrics_channels", "track_id = ?1"),
                            children(
                                "channel_spans",
                                "owner_id IN (SELECT id FROM lyrics_channels WHERE track_id = ?1)",
                            ),
                        ],
                    ),
                ]
            }
        }
        Kind::Tag => const { &[("aliases", direct!("tag_aliases"))] },
    }
}

async fn columns(db: &mut SqliteConnection, table: &str) -> Result<Vec<String>> {
    Ok(
        query_scalar("SELECT name FROM pragma_table_info(?) ORDER BY cid")
            .bind(table)
            .fetch_all(db)
            .await?,
    )
}

fn object(columns: &[String]) -> String {
    format!(
        "json_object({})",
        columns
            .iter()
            .map(|column| format!("'{column}', \"{column}\""))
            .collect::<Vec<_>>()
            .join(",")
    )
}

pub async fn snapshot(db: &mut SqliteConnection, kind: Kind, id: &str) -> Result<Snapshot> {
    let pk = columns(db, kind.path()).await?;
    let sql = format!("SELECT {} FROM {} WHERE id = ?", object(&pk), kind.path());
    let Some(root): Option<String> = query_scalar(AssertSqlSafe(sql))
        .bind(id)
        .fetch_optional(&mut *db)
        .await?
    else {
        return Ok(Snapshot::new());
    };
    let mut fields: Snapshot = serde_json::from_str(&root)?;
    fields.remove("id");
    for &(field, tables) in groups(kind) {
        let mut data = Map::new();
        for table in tables {
            let cols = columns(db, table.name).await?;
            let sql = format!(
                "SELECT {} FROM {} WHERE {} ORDER BY {}",
                object(&cols),
                table.name,
                table.scope,
                cols.iter()
                    .map(|col| format!("\"{col}\""))
                    .collect::<Vec<_>>()
                    .join(",")
            );
            let rows: Vec<String> = query_scalar(AssertSqlSafe(sql))
                .bind(id)
                .fetch_all(&mut *db)
                .await?;
            let values = rows
                .iter()
                .map(|row| serde_json::from_str(row))
                .collect::<serde_json::Result<Vec<Value>>>()?;
            data.insert(table.name.into(), Value::Array(values));
        }
        fields.insert(field.into(), Value::Object(data));
    }
    Ok(fields)
}

pub fn missing(value: &Value) -> bool {
    match value {
        Value::Null => true,
        Value::String(text) => text.trim().is_empty(),
        Value::Object(tables) => tables
            .values()
            .all(|value| value.as_array().is_some_and(Vec::is_empty)),
        _ => false,
    }
}

pub async fn audit(
    db: &mut SqliteConnection,
    kind: Kind,
    id: &str,
    before: &Snapshot,
) -> Result<()> {
    let after = snapshot(db, kind, id).await?;
    for (field, value) in after {
        let previous = before.get(&field).unwrap_or(&Value::Null);
        if previous == &value {
            continue;
        }
        query(
            "INSERT INTO record_history (record_id, field, minute, old_value, new_value)
            VALUES (?, ?, unixepoch() / 60, ?, ?)
            ON CONFLICT(record_id, field, minute) DO UPDATE SET new_value = excluded.new_value",
        )
        .bind(id)
        .bind(field)
        .bind(previous.to_string())
        .bind(value.to_string())
        .execute(&mut *db)
        .await?;
    }
    Ok(())
}

pub async fn seed(db: &mut SqliteConnection, kind: Kind, id: &str) -> Result<()> {
    for (field, value) in snapshot(db, kind, id).await? {
        if missing(&value)
            || (field == "discs"
                && value
                    .get("disc_tracks")
                    .and_then(Value::as_array)
                    .is_some_and(Vec::is_empty))
            || (field == "duration_ms" && value.as_u64() == Some(0))
            || (field == "kind" && value.as_str() == Some("Unknown"))
        {
            queue(db, id, &field, false).await?;
        }
    }
    Ok(())
}

pub async fn queue(db: &mut SqliteConnection, id: &str, field: &str, refresh: bool) -> Result<()> {
    query("INSERT INTO enrichment_needed (record_id, field, due_at, refresh, generation) VALUES (?, ?, unixepoch(), ?, random())
        ON CONFLICT(record_id, field) DO UPDATE SET due_at = excluded.due_at,
            generation = excluded.generation, refresh = excluded.refresh WHERE excluded.refresh")
        .bind(id).bind(field).bind(refresh).execute(db).await?;
    Ok(())
}

pub async fn reconcile_source(
    db: &mut SqliteConnection,
    kind: Kind,
    id: &str,
    before: &Snapshot,
    fields: &BTreeMap<&str, Option<Value>>,
) -> Result<()> {
    let after = snapshot(db, kind, id).await?;
    for (field, previous) in before {
        if let Some(proposed) = fields.get(field.as_str()) {
            if let Some(proposed) = proposed.as_ref().filter(|value| !value.is_null()) {
                set_field(db, kind, id, field, proposed).await?;
            }
        } else if after.get(field) != Some(previous) {
            set_field(db, kind, id, field, previous).await?;
        }
    }
    verify(db, kind, id).await?;
    audit(db, kind, id, before).await
}

#[derive(sqlx::FromRow)]
pub struct Revision {
    pub field: String,
    pub minute: i64,
    pub old_value: String,
    pub new_value: String,
}

impl Store {
    pub async fn save_lyrics(&self, track: &str, language: &str, text: &str) -> Result<()> {
        let language = language.trim();
        if language.is_empty()
            || language.len() > 35
            || !language.split('-').all(|part| {
                !part.is_empty() && part.bytes().all(|byte| byte.is_ascii_alphanumeric())
            })
        {
            return Err(Error::msg("Enter a language tag such as en or pt-BR"));
        }
        let mut db = self.0.begin_with("BEGIN IMMEDIATE").await?;
        let before = snapshot(&mut db, Kind::Track, track).await?;
        if before.is_empty() {
            return Err(Error::msg("Unknown track"));
        }
        query("DELETE FROM track_lyrics WHERE track_id = ? AND language = ?")
            .bind(track)
            .bind(language)
            .execute(&mut *db)
            .await?;
        if !text.trim().is_empty() {
            query("INSERT INTO track_lyrics (track_id, language, instrumental) VALUES (?, ?, 0)")
                .bind(track)
                .bind(language)
                .execute(&mut *db)
                .await?;
            let channel: i64 = query_scalar(
                "INSERT INTO lyrics_channels (track_id, language, position, kind)
                VALUES (?, ?, 0, 'Lead') RETURNING id",
            )
            .bind(track)
            .bind(language)
            .fetch_one(&mut *db)
            .await?;
            query("INSERT INTO channel_spans (owner_id, position, text) VALUES (?, 0, ?)")
                .bind(channel)
                .bind(text)
                .execute(&mut *db)
                .await?;
        }
        audit(&mut db, Kind::Track, track, &before).await?;
        clear_job(&mut db, track, "lyrics").await?;
        db.commit().await?;
        Ok(())
    }

    pub async fn editor(
        &self,
        kind: Kind,
        id: &str,
    ) -> Result<(Snapshot, Vec<Revision>, Vec<String>)> {
        let mut db = self.0.begin().await?;
        let fields = snapshot(&mut db, kind, id).await?;
        let revisions = sqlx::query_as(
            "SELECT field, minute, old_value, new_value FROM record_history
            WHERE record_id = ? AND old_value != new_value ORDER BY minute DESC, field",
        )
        .bind(id)
        .fetch_all(&mut *db)
        .await?;
        let pending = query_scalar("SELECT field FROM enrichment_needed WHERE record_id = ?")
            .bind(id)
            .fetch_all(&mut *db)
            .await?;
        db.commit().await?;
        Ok((fields, revisions, pending))
    }

    pub async fn edit(
        &self,
        kind: Kind,
        id: &str,
        action: &str,
        field: &str,
        input: &str,
    ) -> Result<()> {
        let mut db = self.0.begin_with("BEGIN IMMEDIATE").await?;
        let before = snapshot(&mut db, kind, id).await?;
        if before.is_empty() || (!field.is_empty() && !before.contains_key(field)) {
            return Err(Error::msg("Unknown record or field"));
        }
        match action {
            "queue" => {
                for key in before
                    .keys()
                    .filter(|key| field.is_empty() || key.as_str() == field)
                {
                    queue(&mut db, id, key, true).await?;
                }
            }
            "save" => {
                if field.is_empty() {
                    return Err(Error::msg("Select a field to edit"));
                }
                set_field(&mut db, kind, id, field, &serde_json::from_str(input)?).await?;
                verify(&mut db, kind, id).await?;
                audit(&mut db, kind, id, &before).await?;
                clear_job(&mut db, id, field).await?;
            }
            "save_all" => {
                let fields: Snapshot = serde_json::from_str(input)?;
                for (key, value) in &fields {
                    if before.get(key) != Some(value) {
                        set_field(&mut db, kind, id, key, value).await?;
                        clear_job(&mut db, id, key).await?;
                    }
                }
                verify(&mut db, kind, id).await?;
                audit(&mut db, kind, id, &before).await?;
            }
            "restore" => {
                let minute: i64 = input.parse()?;
                let changes: Vec<Revision> = sqlx::query_as(
                    "SELECT field, minute, old_value, new_value FROM record_history
                    WHERE record_id = ? AND minute > ? AND (? = '' OR field = ?)
                    ORDER BY minute DESC, field",
                )
                .bind(id)
                .bind(minute)
                .bind(field)
                .bind(field)
                .fetch_all(&mut *db)
                .await?;
                let changes = changes
                    .into_iter()
                    .map(|value| (value.field, value.old_value))
                    .collect::<BTreeMap<_, _>>();
                for (key, value) in changes {
                    set_field(&mut db, kind, id, &key, &serde_json::from_str(&value)?).await?;
                    clear_job(&mut db, id, &key).await?;
                }
                verify(&mut db, kind, id).await?;
                audit(&mut db, kind, id, &before).await?;
            }
            _ => return Err(Error::msg("Unknown editor action")),
        }
        db.commit().await?;
        Ok(())
    }
}

async fn verify(db: &mut SqliteConnection, kind: Kind, id: &str) -> Result<()> {
    use crate::records::{DatabaseRecord, read_by_id};
    match kind {
        Kind::Artist => {
            read_by_id::<concentus_types::Artist>(db, id).await?;
        }
        Kind::Album => {
            read_by_id::<concentus_types::Album>(db, id).await?;
        }
        Kind::Track => {
            if let Some(mut track) = read_by_id::<concentus_types::Track>(db, id).await? {
                track.load_lyrics(db).await?;
            }
        }
        Kind::Tag => {
            read_by_id::<concentus_types::Tag>(db, id).await?;
        }
    }
    Ok(())
}

async fn set_field(
    db: &mut SqliteConnection,
    kind: Kind,
    id: &str,
    field: &str,
    value: &Value,
) -> Result<()> {
    if let Some((_, tables)) = groups(kind).iter().find(|(key, _)| *key == field) {
        for table in tables.iter().rev() {
            let sql = format!("DELETE FROM {} WHERE {}", table.name, table.scope);
            query(AssertSqlSafe(sql)).bind(id).execute(&mut *db).await?;
        }
        if value.is_null() {
            return Ok(());
        }
        let data = value
            .as_object()
            .ok_or_else(|| Error::msg("Expected a table object"))?;
        if data
            .keys()
            .any(|key| !tables.iter().any(|table| table.name == key))
        {
            return Err(Error::msg("Unknown child table"));
        }
        for table in *tables {
            let Some(values) = data.get(table.name) else {
                continue;
            };
            let rows = values
                .as_array()
                .ok_or_else(|| Error::msg("Expected an array of rows"))?;
            let cols = columns(db, table.name).await?;
            for value in rows {
                let mut row = value
                    .as_object()
                    .ok_or_else(|| Error::msg("Expected a row object"))?
                    .clone();
                if let Some((root, _)) = table.root.split_once(' ') {
                    row.insert(root.into(), Value::String(id.into()));
                } else {
                    let owner = row
                        .get("owner_id")
                        .ok_or_else(|| Error::msg("Child row needs an owner ID"))?;
                    let predicate = table
                        .scope
                        .replace("owner_id IN", "json_extract(?2, '$') IN");
                    let sql = format!("SELECT EXISTS(SELECT 1 WHERE {predicate})");
                    let belongs_to_record: bool = query_scalar(AssertSqlSafe(sql))
                        .bind(id)
                        .bind(scalar(owner)?)
                        .fetch_one(&mut *db)
                        .await?;
                    if !belongs_to_record {
                        return Err(Error::msg("Child row belongs to another record"));
                    }
                }
                if row.is_empty() || row.keys().any(|key| !cols.contains(key)) {
                    return Err(Error::msg("Unknown or empty column list"));
                }
                let mut sql = QueryBuilder::<Sqlite>::new(format!(
                    "INSERT INTO {} ({}) VALUES (",
                    table.name,
                    row.keys()
                        .map(|key| format!("\"{key}\""))
                        .collect::<Vec<_>>()
                        .join(",")
                ));
                {
                    let mut values = sql.separated(",");
                    for value in row.values() {
                        let scalar = scalar(value)?;
                        values
                            .push("json_extract(")
                            .push_bind_unseparated(scalar)
                            .push_unseparated(", '$')");
                    }
                }
                sql.push(")");
                sql.build().execute(&mut *db).await?;
            }
        }
        return Ok(());
    }
    if field == "id"
        || !columns(db, kind.path())
            .await?
            .iter()
            .any(|column| column == field)
    {
        return Err(Error::msg("Unknown scalar field"));
    }
    let sql = format!(
        "UPDATE {} SET \"{field}\" = json_extract(?, '$') WHERE id = ?",
        kind.path()
    );
    query(AssertSqlSafe(sql))
        .bind(scalar(value)?)
        .bind(id)
        .execute(db)
        .await?;
    Ok(())
}

fn scalar(value: &Value) -> Result<String> {
    if value.is_array() || value.is_object() {
        return Err(Error::msg("Expected text, a number, a boolean or null"));
    }
    Ok(value.to_string())
}

pub async fn finish_import(
    db: &mut SqliteConnection,
    kind: Kind,
    id: &str,
    before: &Snapshot,
) -> Result<()> {
    if before.is_empty() {
        seed(db, kind, id).await
    } else {
        audit(db, kind, id, before).await
    }
}
async fn clear_job(db: &mut SqliteConnection, id: &str, field: &str) -> Result<()> {
    query("DELETE FROM enrichment_needed WHERE record_id = ? AND field = ?")
        .bind(id)
        .bind(field)
        .execute(db)
        .await?;
    Ok(())
}
