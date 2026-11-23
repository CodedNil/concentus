//! Imports provider results into the local catalog before they are shown.

pub mod cover_art_archive;
mod import;
mod lrclib;
mod musicbrainz;
mod wikidata;
mod wikimedia_commons;

use crate::{catalog::Kind, store::Store};
use musicbrainz::{Batch, MusicBrainz};
use std::{sync::Arc, time::Duration};
use tokio::{
    sync::{Mutex, mpsc},
    time::{Instant, timeout_at},
};
use topcoat::Result;

#[derive(Clone)]
pub struct Enrichment {
    musicbrainz: MusicBrainz,
    search: Arc<Mutex<()>>,
}

pub enum Update {
    Results(Kind, Vec<String>),
    Unavailable,
}

impl Enrichment {
    pub fn new() -> Result<Self> {
        Ok(Self {
            musicbrainz: MusicBrainz::new()?,
            search: Arc::new(Mutex::new(())),
        })
    }

    pub fn search(&self, store: Store, kinds: Vec<Kind>, query: String) -> mpsc::Receiver<Update> {
        let (updates, receiver) = mpsc::channel(16);
        let enrichment = self.clone();
        tokio::spawn(async move {
            let Ok(_guard) = enrichment.search.try_lock() else {
                let _ = updates.send(Update::Unavailable).await;
                return;
            };
            enrichment.run_search(&store, kinds, &query, &updates).await;
        });
        receiver
    }

    async fn run_search(
        &self,
        store: &Store,
        kinds: Vec<Kind>,
        query: &str,
        updates: &mpsc::Sender<Update>,
    ) {
        let mut album_details = None;
        for kind in kinds {
            match self.results(store, kind, query).await {
                Ok((ids, releases)) => {
                    if !releases.is_empty() {
                        album_details = Some((ids.clone(), releases));
                    }
                    let _ = updates.send(Update::Results(kind, ids)).await;
                }
                Err(error) => {
                    eprintln!("MusicBrainz {} search failed: {error}", kind.path());
                    let _ = updates.send(Update::Unavailable).await;
                }
            }
        }
        if let Some((ids, releases)) = album_details {
            let deadline = Instant::now() + Duration::from_secs(15);
            for id in releases {
                match timeout_at(deadline, self.musicbrainz.release(id)).await {
                    Ok(Ok(release)) => {
                        match import::save(store, Batch::Releases(vec![release])).await {
                            Ok(_) => {
                                let _ = updates
                                    .send(Update::Results(Kind::Album, ids.clone()))
                                    .await;
                            }
                            Err(error) => {
                                eprintln!("MusicBrainz release {id} import failed: {error}");
                            }
                        }
                    }
                    Ok(Err(error)) => eprintln!("MusicBrainz release {id} detail failed: {error}"),
                    Err(_) => break,
                }
            }
        }
    }

    async fn results(
        &self,
        store: &Store,
        kind: Kind,
        query: &str,
    ) -> Result<(Vec<String>, Vec<uuid::Uuid>)> {
        let batch = self.musicbrainz.search(kind, query).await?;
        let releases = match &batch {
            Batch::Releases(releases) => releases
                .iter()
                .filter_map(|release| release.id.parse().ok())
                .collect(),
            _ => Vec::new(),
        };
        Ok((import::save(store, batch).await?, releases))
    }
}
