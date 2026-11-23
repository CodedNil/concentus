use std::path::Path;

use crate::{catalog::Catalog, lookup::SqlLookup, records::DatabaseRecord};
use sqlx::{
    AssertSqlSafe, Row, Sqlite, SqlitePool,
    migrate::Migrator,
    sqlite::{SqliteConnectOptions, SqliteJournalMode},
};
use topcoat::{
    Result,
    context::{Cx, context},
};

static MIGRATIONS: Migrator = sqlx::migrate!();
const MAX_PAGE: u32 = 100;

/// The local catalog database.
pub struct Store(SqlitePool);

impl Store {
    pub async fn open(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent().filter(|path| !path.as_os_str().is_empty()) {
            std::fs::create_dir_all(parent)?;
        }
        let options = SqliteConnectOptions::new()
            .filename(path)
            .create_if_missing(true)
            .journal_mode(SqliteJournalMode::Wal);
        let pool = SqlitePool::connect_with(options).await?;
        MIGRATIONS.run(&pool).await?;
        Ok(Self(pool))
    }

    pub async fn lookup<T: DatabaseRecord>(&self, lookup: &SqlLookup) -> Result<Option<T>> {
        let Some(predicate) = lookup.where_clause() else {
            return Ok(None);
        };
        let cursor_param = lookup.args.len() + 1;
        let limit_param = cursor_param + 1;
        let sql = format!(
            "SELECT record.* FROM {} AS record WHERE ({predicate}) AND id > ?{cursor_param}
                ORDER BY id LIMIT ?{limit_param}",
            T::NAME
        );
        let mut db = self.0.begin().await?;
        let mut cursor = String::new();
        let mut found = None;
        let lyrics_filter = lookup.has_lyrics_filters();
        let limit = if lookup.has_field_filters() {
            MAX_PAGE
        } else {
            2
        };
        let page_size = usize::try_from(limit)?;
        loop {
            // The table and predicate shape are selected by the server; values are bound.
            let mut query = sqlx::query::<Sqlite>(AssertSqlSafe(sql.as_str()));
            for arg in &lookup.args {
                query = query.bind(arg.as_deref());
            }
            let rows = query
                .bind(cursor.as_str())
                .bind(limit)
                .fetch_all(&mut *db)
                .await?;
            if !lookup.has_field_filters() && rows.len() > 1 {
                return Ok(None);
            }
            let full = rows.len() == page_size;
            for row in rows {
                cursor = row.try_get("id")?;
                let mut record = T::read(&mut db, row).await?;
                if lyrics_filter {
                    record.load_lyrics(&mut db).await?;
                }
                if lookup.matches(&record)? {
                    if found.is_some() {
                        return Ok(None);
                    }
                    found = Some(record);
                }
            }
            if !full {
                if let Some(record) = &mut found {
                    match (lookup.include_lyrics, lyrics_filter) {
                        (true, false) => record.load_lyrics(&mut db).await?,
                        (false, true) => record.omit_lyrics(),
                        _ => {}
                    }
                }
                db.commit().await?;
                return Ok(found);
            }
        }
    }

    pub async fn list<T: DatabaseRecord>(&self, after: Option<&str>, limit: u32) -> Result<Vec<T>> {
        self.list_with_lyrics(after, limit, false).await
    }

    async fn list_with_lyrics<T: DatabaseRecord>(
        &self,
        after: Option<&str>,
        limit: u32,
        include_lyrics: bool,
    ) -> Result<Vec<T>> {
        let sql = format!("SELECT * FROM {} WHERE id > ? ORDER BY id LIMIT ?", T::NAME);
        let mut db = self.0.begin().await?;
        let rows = sqlx::query::<Sqlite>(AssertSqlSafe(sql))
            .bind(after.unwrap_or_default())
            .bind(limit.min(MAX_PAGE))
            .fetch_all(&mut *db)
            .await?;
        let mut records = Vec::with_capacity(rows.len());
        for row in rows {
            let mut record = T::read(&mut db, row).await?;
            if include_lyrics {
                record.load_lyrics(&mut db).await?;
            }
            records.push(record);
        }
        db.commit().await?;
        Ok(records)
    }

    pub async fn get<T: DatabaseRecord>(&self, id: &str) -> Result<Option<T>> {
        let sql = format!("SELECT * FROM {} WHERE id = ?", T::NAME);
        let mut db = self.0.begin().await?;
        let row = sqlx::query::<Sqlite>(AssertSqlSafe(sql))
            .bind(id)
            .fetch_optional(&mut *db)
            .await?;
        let record = match row {
            Some(row) => Some(T::read(&mut db, row).await?),
            None => None,
        };
        db.commit().await?;
        Ok(record)
    }

    /// The first page of each collection shown on the catalog page.
    pub async fn catalog(&self) -> Result<Catalog> {
        Ok(Catalog {
            artists: self.list(None, MAX_PAGE).await?,
            albums: self.list(None, MAX_PAGE).await?,
            tracks: self.list_with_lyrics(None, MAX_PAGE, true).await?,
        })
    }
}

pub fn store(cx: &Cx) -> &Store {
    context(cx)
}
