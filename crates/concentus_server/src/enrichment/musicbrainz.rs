use crate::catalog::Kind;
use reqwest::{StatusCode, Url, header::RETRY_AFTER};
use serde::{Deserialize, de::DeserializeOwned};
use std::{env, sync::Arc, time::Duration};
use tokio::sync::Mutex;
use tokio::time::{Instant, sleep_until};
use topcoat::{Error, Result};

#[derive(Clone)]
pub struct MusicBrainz {
    client: reqwest::Client,
    endpoint: Url,
    next_request: Arc<Mutex<Instant>>,
}

impl MusicBrainz {
    pub async fn albums_by_artist(&self, id: uuid::Uuid) -> Result<Vec<Release>> {
        let id = id.to_string();
        let mut releases = Vec::new();
        loop {
            let page: ReleasePage = self
                .request(
                    "release",
                    &[
                        ("artist", &id),
                        ("type", "album"),
                        ("status", "official"),
                        ("inc", "artist-credits+release-groups+labels"),
                        ("limit", "100"),
                        ("offset", &releases.len().to_string()),
                    ],
                )
                .await?;
            if page.releases.is_empty() {
                return Ok(releases);
            }
            releases.extend(page.releases);
            if releases.len() >= page.count {
                return Ok(releases);
            }
        }
    }

    pub async fn entity(&self, kind: Kind, id: uuid::Uuid) -> Result<Batch> {
        Ok(match kind {
            Kind::Artist => Batch::Artists(vec![
                self.request(&format!("artist/{id}"), &[("inc", "tags")])
                    .await?,
            ]),
            Kind::Album => Batch::Releases(vec![self.release(id).await?]),
            Kind::Track => Batch::Recordings(vec![
                self.request(
                    &format!("recording/{id}"),
                    &[("inc", "artist-credits+releases+isrcs+tags")],
                )
                .await?,
            ]),
            Kind::Tag => return Err(Error::msg("MusicBrainz does not resolve Concentus tags")),
        })
    }
    pub async fn wikidata_id(&self, id: uuid::Uuid) -> Result<Option<String>> {
        let artist: ArtistLinks = self
            .request(&format!("artist/{id}"), &[("inc", "url-rels")])
            .await?;
        let mut ids = artist
            .relations
            .into_iter()
            .filter(|relation| relation.kind == "wikidata")
            .filter_map(|relation| {
                let url = Url::parse(&relation.url?.resource).ok()?;
                let id = url.path().strip_prefix("/wiki/")?;
                (matches!(url.host_str(), Some("www.wikidata.org" | "wikidata.org"))
                    && super::wikidata::valid_id(id))
                .then(|| id.to_owned())
            })
            .collect::<Vec<_>>();
        ids.sort_unstable();
        ids.dedup();
        Ok(if ids.len() == 1 { ids.pop() } else { None })
    }

    pub fn new() -> Result<Self> {
        let endpoint = match env::var("CONCENTUS_MUSICBRAINZ_ENDPOINT") {
            Ok(endpoint) => endpoint,
            Err(env::VarError::NotPresent) => "https://musicbrainz.org/ws/2/".into(),
            Err(error) => return Err(error.into()),
        };
        let endpoint = Url::parse(&format!("{}/", endpoint.trim_end_matches('/')))?;
        Ok(Self {
            client: reqwest::Client::builder()
                .user_agent(concat!(
                    "Concentus/",
                    env!("CARGO_PKG_VERSION"),
                    " (https://github.com/CodedNil/concentus)"
                ))
                .timeout(Duration::from_secs(6))
                .build()?,
            endpoint,
            next_request: Arc::new(Mutex::new(Instant::now())),
        })
    }

    async fn request<T: DeserializeOwned>(&self, path: &str, params: &[(&str, &str)]) -> Result<T> {
        let mut next = self.next_request.lock().await;
        let url = self.endpoint.join(path)?;
        let request = self.client.get(url).query(&[("fmt", "json")]).query(params);
        let mut retry = true;
        loop {
            if *next > Instant::now() + Duration::from_secs(3) {
                return Err(Error::msg("MusicBrainz asked us to retry later"));
            }
            sleep_until(*next).await;
            *next = Instant::now() + Duration::from_millis(1100);
            let response = request
                .try_clone()
                .expect("GET request is clonable")
                .send()
                .await
                .map_err(reqwest::Error::without_url)?;
            tracing::debug!(
                provider = "MusicBrainz",
                status = response.status().as_u16(),
                "Provider responded"
            );
            if matches!(
                response.status(),
                StatusCode::TOO_MANY_REQUESTS | StatusCode::SERVICE_UNAVAILABLE
            ) {
                let delay = response
                    .headers()
                    .get(RETRY_AFTER)
                    .and_then(|value| value.to_str().ok())
                    .and_then(|seconds| seconds.parse().ok())
                    .unwrap_or(2);
                *next = (*next).max(Instant::now() + Duration::from_secs(delay.min(3600)));
                if retry && delay <= 3 {
                    retry = false;
                    continue;
                }
            }
            drop(next);
            return Ok(response
                .error_for_status()
                .map_err(reqwest::Error::without_url)?
                .json()
                .await
                .map_err(reqwest::Error::without_url)?);
        }
    }

    pub async fn search(&self, kind: Kind, input: &str) -> Result<Batch> {
        let query = search_query(kind, input);
        let params = [("query", query.as_str()), ("limit", "5")];
        Ok(match kind {
            Kind::Artist => {
                Batch::Artists(self.request::<Search>("artist", &params).await?.artists)
            }
            Kind::Track => Batch::Recordings(
                self.request::<Search>("recording", &params)
                    .await?
                    .recordings,
            ),
            Kind::Album => {
                Batch::Releases(self.request::<Search>("release", &params).await?.releases)
            }
            Kind::Tag => return Err(Error::msg("MusicBrainz does not search Concentus tags")),
        })
    }

    async fn release(&self, id: uuid::Uuid) -> Result<Release> {
        self.request(
            &format!("release/{id}"),
            &[(
                "inc",
                "recordings+artist-credits+release-groups+labels+isrcs+tags",
            )],
        )
        .await
    }
}

#[derive(Deserialize)]
struct ReleasePage {
    #[serde(rename = "release-count")]
    count: usize,
    releases: Vec<Release>,
}

#[derive(Deserialize)]
struct ArtistLinks {
    #[serde(default)]
    relations: Vec<Relation>,
}

#[derive(Deserialize)]
struct Relation {
    #[serde(rename = "type")]
    kind: String,
    url: Option<RelationshipUrl>,
}

#[derive(Deserialize)]
struct RelationshipUrl {
    resource: String,
}

fn search_query(kind: Kind, input: &str) -> String {
    if kind == Kind::Track
        && let Ok(isrc) = input.parse::<concentus_types::Isrc>()
    {
        return format!("isrc:{}", isrc.as_str());
    }
    let field = match kind {
        Kind::Artist => "artist",
        Kind::Album => "release",
        Kind::Track => "recording",
        Kind::Tag => "tag",
    };
    let quote = |value: &str| {
        let mut escaped = String::from("\"");
        for ch in value.trim().chars() {
            if matches!(ch, '\\' | '"') {
                escaped.push('\\');
            }
            escaped.push(ch);
        }
        escaped.push('"');
        escaped
    };
    if kind != Kind::Artist
        && let Some((artist, title)) = input.split_once(" — ").or_else(|| input.split_once(" - "))
        && !artist.trim().is_empty()
        && !title.trim().is_empty()
    {
        return format!("{field}:{} AND artist:{}", quote(title), quote(artist));
    }
    if matches!(kind, Kind::Album | Kind::Track) && input.split_whitespace().count() > 1 {
        let terms = input
            .split_whitespace()
            .take(16)
            .map(|word| {
                let word = quote(word);
                format!("({field}:{word} OR artist:{word})")
            })
            .collect::<Vec<_>>()
            .join(" AND ");
        return format!("{field}:{}^4 OR ({terms})", quote(input));
    }
    format!("{field}:{}", quote(input))
}

pub enum Batch {
    Artists(Vec<Artist>),
    Releases(Vec<Release>),
    Recordings(Vec<Recording>),
}

#[derive(Default, Deserialize)]
#[serde(default)]
struct Search {
    artists: Vec<Artist>,
    releases: Vec<Release>,
    recordings: Vec<Recording>,
}

#[derive(Deserialize)]
pub struct Artist {
    pub id: String,
    pub name: String,
    #[serde(rename = "sort-name")]
    pub sort_name: Option<String>,
    #[serde(rename = "type")]
    pub kind: Option<String>,
    #[serde(rename = "life-span")]
    pub lifespan: Option<Lifespan>,
    #[serde(rename = "begin-area")]
    pub begin_area: Option<Area>,
    pub area: Option<Area>,
    #[serde(default)]
    pub isnis: Vec<String>,
    #[serde(default)]
    pub ipis: Vec<String>,
    #[serde(default)]
    pub tags: Vec<SourceTag>,
}

#[derive(Deserialize)]
pub struct Lifespan {
    pub begin: Option<String>,
    pub end: Option<String>,
}

#[derive(Deserialize)]
pub struct Area {
    pub name: String,
}

#[derive(Deserialize)]
pub struct SourceTag {
    pub name: String,
    pub count: Option<i64>,
}

#[derive(Deserialize)]
pub struct Credit {
    pub artist: Artist,
}

#[derive(Deserialize)]
pub struct Release {
    pub id: String,
    pub title: String,
    pub date: Option<String>,
    pub status: Option<String>,
    pub barcode: Option<String>,
    #[serde(rename = "artist-credit", default)]
    pub credits: Vec<Credit>,
    #[serde(rename = "release-group")]
    pub group: Option<ReleaseGroup>,
    #[serde(rename = "label-info", default)]
    pub labels: Vec<LabelInfo>,
    #[serde(rename = "cover-art-archive")]
    pub cover_art: Option<CoverArt>,
    #[serde(default)]
    pub media: Vec<Medium>,
    #[serde(default)]
    pub tags: Vec<SourceTag>,
}

#[derive(Deserialize)]
pub struct ReleaseGroup {
    pub id: Option<String>,
    #[serde(rename = "primary-type")]
    pub primary_type: Option<String>,
    #[serde(rename = "secondary-types", default)]
    pub secondary_types: Vec<String>,
}

#[derive(Deserialize)]
pub struct LabelInfo {
    pub label: Option<Area>,
    #[serde(rename = "catalog-number")]
    pub catalog_number: Option<String>,
}

#[derive(Deserialize)]
pub struct CoverArt {
    pub front: bool,
}

#[derive(Deserialize)]
pub struct Medium {
    pub position: Option<i64>,
    pub title: Option<String>,
    #[serde(default)]
    pub tracks: Vec<MediumTrack>,
}

#[derive(Deserialize)]
pub struct MediumTrack {
    pub id: String,
    pub position: i64,
    pub length: Option<u32>,
    pub recording: Recording,
    #[serde(rename = "artist-credit", default)]
    pub credits: Vec<Credit>,
}

#[derive(Deserialize)]
pub struct Recording {
    pub id: String,
    pub title: String,
    pub length: Option<u32>,
    #[serde(rename = "first-release-date")]
    pub date: Option<String>,
    #[serde(rename = "artist-credit", default)]
    pub credits: Vec<Credit>,
    #[serde(default)]
    pub releases: Vec<Release>,
    #[serde(default)]
    pub isrcs: Vec<String>,
    #[serde(default)]
    pub tags: Vec<SourceTag>,
}
