//! Async HTTP client for the shared Concentus Postcard API.

pub use concentus_types as types;
use concentus_types::{
    Album, AlbumLookup, ApiError, Artist, ArtistLookup, CONTENT_TYPE, CodecError, Collection, Id,
    Lookup, PageQuery, Tag, TagLookup, Track, TrackLookup, decode, encode,
};
use reqwest::{RequestBuilder, Response, StatusCode, Url, header};
use serde::de::DeserializeOwned;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("expected an HTTP or HTTPS base URL without a query or fragment")]
    InvalidBaseUrl,
    #[error("HTTP request failed: {0}")]
    Http(#[from] reqwest::Error),
    #[error("Postcard codec error: {0}")]
    Codec(#[from] CodecError),
    #[error("unexpected response from Concentus ({0})")]
    UnexpectedResponse(StatusCode),
    #[error("Concentus returned {status}: {message}")]
    Server { status: StatusCode, message: String },
}

#[derive(Clone, Debug)]
pub struct Client {
    http: reqwest::Client,
    base_url: Url,
}

impl Client {
    pub fn new(base_url: &str) -> Result<Self, Error> {
        Self::with_http_client(base_url, reqwest::Client::builder().build()?)
    }

    /// Reuse an HTTP client configured by the application.
    pub fn with_http_client(base_url: &str, http: reqwest::Client) -> Result<Self, Error> {
        let base_url = Url::parse(base_url).map_err(|_| Error::InvalidBaseUrl)?;
        if !matches!(base_url.scheme(), "http" | "https")
            || base_url.cannot_be_a_base()
            || base_url.query().is_some()
            || base_url.fragment().is_some()
        {
            return Err(Error::InvalidBaseUrl);
        }
        Ok(Self { http, base_url })
    }

    pub async fn list<T: Collection>(&self, query: &PageQuery) -> Result<Vec<T>, Error> {
        let url = self.endpoint(&[T::NAME])?;
        read_response(send(self.http.get(url).query(query)).await?).await
    }

    pub async fn get<T: Collection>(&self, id: Id<T>) -> Result<Option<T>, Error> {
        let url = self.endpoint(&[T::NAME, &id.to_string()])?;
        let response = send(self.http.get(url)).await?;
        if response.status() == StatusCode::NOT_FOUND {
            return Ok(None);
        }
        read_response(response).await.map(Some)
    }

    pub async fn lookup<T: Collection>(&self, request: Lookup<T>) -> Result<Option<T>, Error> {
        let url = self.endpoint(&[T::NAME, "lookup"])?;
        let request = self
            .http
            .post(url)
            .header(header::CONTENT_TYPE, CONTENT_TYPE)
            .body(encode(&request)?);
        read_response(send(request).await?).await
    }

    pub async fn lookup_artist(&self, request: ArtistLookup) -> Result<Option<Artist>, Error> {
        self.lookup(request).await
    }

    pub async fn lookup_album(&self, request: AlbumLookup) -> Result<Option<Album>, Error> {
        self.lookup(request).await
    }

    pub async fn lookup_track(&self, request: TrackLookup) -> Result<Option<Track>, Error> {
        self.lookup(request).await
    }

    pub async fn lookup_tag(&self, request: TagLookup) -> Result<Option<Tag>, Error> {
        self.lookup(request).await
    }

    fn endpoint(&self, path: &[&str]) -> Result<Url, Error> {
        let mut url = self.base_url.clone();
        url.path_segments_mut()
            .map_err(|()| Error::InvalidBaseUrl)?
            .pop_if_empty()
            .push("api")
            .extend(path.iter().copied());
        Ok(url)
    }
}

async fn send(request: RequestBuilder) -> Result<Response, Error> {
    let response = request.header(header::ACCEPT, CONTENT_TYPE).send().await?;
    let content_type = response
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.split(';').next());
    if content_type.is_some_and(|value| value.trim().eq_ignore_ascii_case(CONTENT_TYPE)) {
        Ok(response)
    } else {
        Err(Error::UnexpectedResponse(response.status()))
    }
}

async fn read_response<T: DeserializeOwned>(response: Response) -> Result<T, Error> {
    let status = response.status();
    let body = response.bytes().await?;
    if status.is_success() {
        Ok(decode(&body)?)
    } else {
        let error: ApiError = decode(&body)?;
        Err(Error::Server {
            status,
            message: error.message,
        })
    }
}
