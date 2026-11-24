//! Keyless portrait source: <https://www.mediawiki.org/wiki/API:Imageinfo>.

use super::http::Source;
use crate::store::Store;
use reqwest::Url;
use serde::Deserialize;
use sqlx::{query, query_scalar};
use std::collections::HashMap;
use topcoat::Result;

#[derive(Deserialize)]
struct Response {
    query: Option<Query>,
}

#[derive(Deserialize)]
struct Query {
    pages: Vec<Page>,
}

#[derive(Deserialize)]
struct Page {
    #[serde(default)]
    imageinfo: Vec<Image>,
}

#[derive(Deserialize)]
struct Image {
    thumburl: Option<String>,
    thumbwidth: Option<u16>,
    thumbheight: Option<u16>,
    #[serde(default)]
    extmetadata: HashMap<String, Metadata>,
}

#[derive(Deserialize)]
struct Metadata {
    value: String,
}

pub async fn enrich(source: &Source, store: &Store, artist: &str, file: &str) -> Result<bool> {
    if file.contains('|') {
        return Ok(false);
    }
    let title = format!("File:{file}");
    let Some(response): Option<Response> = source
        .get(&[
            ("action", "query"),
            ("prop", "imageinfo"),
            ("format", "json"),
            ("formatversion", "2"),
            ("titles", &title),
            ("iiprop", "url|size|extmetadata"),
            ("iiurlwidth", "500"),
            ("iiextmetadatafilter", "AttributionRequired"),
        ])
        .await?
    else {
        return Ok(false);
    };
    let Some(image) = response.query.and_then(|query| {
        query
            .pages
            .into_iter()
            .flat_map(|page| page.imageinfo)
            .next()
    }) else {
        return Ok(false);
    };
    let (Some(url), Some(width), Some(height)) = (
        image.thumburl.as_deref().filter(|url| {
            Url::parse(url).is_ok_and(|url| {
                url.scheme() == "https" && url.host_str() == Some("upload.wikimedia.org")
            })
        }),
        image.thumbwidth,
        image.thumbheight,
    ) else {
        return Ok(false);
    };
    if width == 0
        || height == 0
        || image
            .extmetadata
            .get("AttributionRequired")
            .is_none_or(|entry| entry.value.trim() != "false")
    {
        return Ok(false);
    }
    let mut db = store.0.begin_with("BEGIN IMMEDIATE").await?;
    let before = crate::editor::snapshot(&mut db, crate::catalog::Kind::Artist, artist).await?;
    let artwork: Option<i64> = query_scalar(
        "INSERT INTO artwork (owner_id, position, kind, explicit)
        SELECT ?, coalesce(max(position) + 1, 0), 'Portrait', 0 FROM artwork WHERE owner_id = ?
        HAVING NOT EXISTS (SELECT 1 FROM artwork WHERE owner_id = ? AND kind = 'Portrait')
        RETURNING id",
    )
    .bind(artist)
    .bind(artist)
    .bind(artist)
    .fetch_optional(&mut *db)
    .await?;
    if let Some(artwork) = artwork {
        query("INSERT INTO artwork_renditions (owner_id, position, url, width, height) VALUES (?, 0, ?, ?, ?)")
            .bind(artwork).bind(url).bind(width).bind(height).execute(&mut *db).await?;
    }
    crate::editor::audit(&mut db, crate::catalog::Kind::Artist, artist, &before).await?;
    db.commit().await?;
    Ok(artwork.is_some())
}
