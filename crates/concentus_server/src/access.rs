use crate::store::Store;
use sqlx::query;
use std::{
    collections::HashMap,
    sync::{Mutex, PoisonError},
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use topcoat::Result;

const SECONDS_PER_DAY: u64 = 86_400;

#[derive(Default)]
pub struct AccessLog(Mutex<HashMap<(String, u64), Totals>>);

#[derive(Default)]
struct Totals {
    count: u64,
    last_access: u64,
}

impl AccessLog {
    pub fn record(&self, id: &str) {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let mut counts = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        let entry = counts
            .entry((id.to_owned(), now / SECONDS_PER_DAY))
            .or_default();
        entry.count += 1;
        entry.last_access = entry.last_access.max(now);
        drop(counts);
    }

    pub fn worker(
        store: Store,
        mut stop: tokio::sync::oneshot::Receiver<()>,
    ) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    () = tokio::time::sleep(Duration::from_secs(5)) => {},
                    _ = &mut stop => break,
                }
                store.flush_access().await;
            }
        })
    }

    pub async fn flush(&self, store: &Store) {
        let counts = std::mem::take(&mut *self.0.lock().unwrap_or_else(PoisonError::into_inner));
        if counts.is_empty() {
            return;
        }
        if let Err(error) = Self::write(store, &counts).await {
            tracing::warn!(%error, "Could not flush access counts");
            let mut pending = self.0.lock().unwrap_or_else(PoisonError::into_inner);
            for (key, totals) in counts {
                let entry = pending.entry(key).or_default();
                entry.count += totals.count;
                entry.last_access = entry.last_access.max(totals.last_access);
            }
            drop(pending);
        }
    }

    async fn write(store: &Store, counts: &HashMap<(String, u64), Totals>) -> Result<()> {
        let mut db = store.0.begin().await?;
        for ((id, day), totals) in counts {
            query("UPDATE records SET last_accessed_at = max(coalesce(last_accessed_at, 0), ?) WHERE id = ?")
                .bind(i64::try_from(totals.last_access)?).bind(id).execute(&mut *db).await?;
            query("INSERT INTO record_access_daily (record_id, day, count)
                   SELECT id, ?, ? FROM records WHERE id = ?
                   ON CONFLICT(record_id, day) DO UPDATE SET count = record_access_daily.count + excluded.count")
                .bind(i64::try_from(*day)?).bind(i64::try_from(totals.count)?).bind(id).execute(&mut *db).await?;
        }
        db.commit().await?;
        Ok(())
    }
}
