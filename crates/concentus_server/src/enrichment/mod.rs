//! Imports provider results into the local catalog before they are shown.

pub mod cover_art_archive;
mod http;
mod import;
mod lrclib;
mod musicbrainz;
mod queue;
mod wikidata;
mod wikimedia_commons;

use crate::{catalog::Kind, store::Store};
use http::Source;
use musicbrainz::MusicBrainz;
use std::{sync::Arc, time::Duration};
use tokio::{
    sync::{Mutex, Notify, mpsc},
    time::timeout,
};
use topcoat::Result;

#[derive(Clone)]
pub struct Enrichment {
    musicbrainz: MusicBrainz,
    lrclib: Source,
    wikidata: Source,
    commons: Source,
    search: Arc<Mutex<()>>,
    wake: Arc<Notify>,
}

pub enum Update {
    Results(Kind, Vec<String>),
    Unavailable,
}

impl Enrichment {
    pub fn new() -> Result<Self> {
        let http = reqwest::Client::builder()
            .user_agent(concat!(
                "Concentus/",
                env!("CARGO_PKG_VERSION"),
                " (https://github.com/CodedNil/concentus)"
            ))
            .timeout(Duration::from_secs(6))
            .build()?;
        Ok(Self {
            musicbrainz: MusicBrainz::new()?,
            lrclib: Source::new(
                &http,
                "CONCENTUS_LRCLIB_ENDPOINT",
                "https://lrclib.net/api/get",
            )?,
            wikidata: Source::new(
                &http,
                "CONCENTUS_WIKIDATA_ENDPOINT",
                "https://www.wikidata.org/w/api.php",
            )?,
            commons: Source::new(
                &http,
                "CONCENTUS_COMMONS_ENDPOINT",
                "https://commons.wikimedia.org/w/api.php",
            )?,
            search: Arc::new(Mutex::new(())),
            wake: Arc::new(Notify::new()),
        })
    }

    pub fn wake_worker(&self) {
        self.wake.notify_one();
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
        for kind in kinds {
            match self.search_import(store, kind, query).await {
                Ok(ids) => {
                    let _ = updates.send(Update::Results(kind, ids)).await;
                }
                Err(error) => {
                    eprintln!("MusicBrainz {} search failed: {error}", kind.path());
                    let _ = updates.send(Update::Unavailable).await;
                }
            }
        }
    }

    async fn search_import(&self, store: &Store, kind: Kind, query: &str) -> Result<Vec<String>> {
        let batch = self.musicbrainz.search(kind, query).await?;
        import::save(store, batch).await
    }

    pub async fn discover(&self, store: &Store, kind: Kind, seed: Discovery) {
        let operation = async {
            match seed {
                Discovery::Search(query) => {
                    self.search_import(store, kind, &query).await?;
                }
                Discovery::Id(id) => {
                    import::save(store, self.musicbrainz.entity(kind, id).await?).await?;
                }
            }
            Ok::<(), topcoat::Error>(())
        };
        if let Ok(Err(error)) = timeout(Duration::from_secs(4), operation).await {
            eprintln!("MusicBrainz API lookup failed: {error}");
        }
    }
}

pub enum Discovery {
    Search(String),
    Id(uuid::Uuid),
}

impl Discovery {
    pub fn from_lookup<T>(kind: Kind, request: &concentus_types::Lookup<T>) -> Option<Self> {
        use concentus_types::{ExternalIdProvider, FieldValue, PathSegment};
        if !Kind::MUSIC.contains(&kind) || request.id.is_some() {
            return None;
        }
        for id in &request.external_ids {
            if id.provider == ExternalIdProvider::Musicbrainz {
                return id.value.parse().ok().map(Self::Id);
            }
        }
        if let Some(id) = request
            .external_ids
            .iter()
            .find(|id| id.provider == ExternalIdProvider::Isrc)
        {
            return (kind == Kind::Track).then(|| Self::Search(id.value.clone()));
        }
        if !request.external_ids.is_empty() {
            return None;
        }
        let title = request.fields.iter().find_map(|filter| {
            if let ([PathSegment::Field(field)], FieldValue::Text(value)) =
                (filter.path.as_slice(), &filter.value)
                && field == kind.title_column()
            {
                return Some(value.as_str());
            }
            None
        });
        let query = match (title, request.artist_name.as_deref(), kind) {
            (Some(title), Some(artist), Kind::Album | Kind::Track) => format!("{artist} - {title}"),
            (Some(title), _, _) => title.to_owned(),
            (None, Some(artist), Kind::Artist) => artist.to_owned(),
            _ => return None,
        };
        let query = query.trim();
        (!query.is_empty() && query.chars().take(161).count() <= 160)
            .then(|| Self::Search(query.to_owned()))
    }
}
