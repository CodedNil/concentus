//! Keyless artist metadata source: <https://www.mediawiki.org/wiki/Wikibase/API>.

use super::{http::Source, musicbrainz::MusicBrainz};
use crate::store::Store;
use concentus_types::PartialDate;
use serde::Deserialize;
use serde_json::Value;
use sqlx::query;
use std::collections::HashMap;
use topcoat::{Error, Result};

#[derive(sqlx::FromRow)]
struct Candidate {
    kind: String,
    wikidata: Option<String>,
    musicbrainz: Option<String>,
    has_portrait: bool,
}

#[derive(Deserialize)]
struct Response {
    #[serde(default)]
    entities: HashMap<String, Entity>,
    error: Option<Value>,
}

#[derive(Deserialize)]
struct Entity {
    missing: Option<serde::de::IgnoredAny>,
    #[serde(default)]
    descriptions: HashMap<String, Text>,
    #[serde(default)]
    claims: HashMap<String, Vec<Claim>>,
}

#[derive(Deserialize)]
struct Text {
    value: String,
}

#[derive(Deserialize)]
struct Claim {
    rank: String,
    mainsnak: MainSnak,
}

#[derive(Deserialize)]
struct MainSnak {
    datavalue: Option<DataValue>,
}

#[derive(Deserialize)]
struct DataValue {
    value: Value,
}

impl Entity {
    fn values(&self, key: &str) -> impl Iterator<Item = &Value> {
        let statements = self.claims.get(key).map_or(&[][..], Vec::as_slice);
        ["preferred", "normal"].into_iter().flat_map(move |rank| {
            statements
                .iter()
                .filter(move |claim| claim.rank == rank)
                .filter_map(|claim| claim.mainsnak.datavalue.as_ref().map(|data| &data.value))
        })
    }

    fn date(&self, key: &str) -> Option<String> {
        self.values(key).find_map(|value| {
            if value.get("calendarmodel")?.as_str()?.rsplit('/').next()? != "Q1985727" {
                return None;
            }
            let (date, _) = value
                .get("time")?
                .as_str()?
                .strip_prefix('+')?
                .split_once('T')?;
            let length = match value.get("precision")?.as_u64()? {
                9 => 4,
                10 => 7,
                11 => 10,
                _ => return None,
            };
            let date: PartialDate = date.get(..length)?.parse().ok()?;
            Some(date.to_string())
        })
    }
}

pub fn valid_id(id: &str) -> bool {
    id.strip_prefix('Q').is_some_and(|digits| {
        !digits.is_empty() && digits.bytes().all(|byte| byte.is_ascii_digit())
    })
}

pub async fn enrich(
    source: &Source,
    musicbrainz: &MusicBrainz,
    store: &Store,
    id: &str,
    fields: &[&str],
) -> Result<Option<String>> {
    let candidate: Option<Candidate> = sqlx::query_as(
        "SELECT artist.kind,
        (SELECT value FROM external_ids WHERE owner_id = artist.id AND provider = 'Wikidata' ORDER BY value LIMIT 1) AS wikidata,
        (SELECT value FROM external_ids WHERE owner_id = artist.id AND provider = 'Musicbrainz' ORDER BY value LIMIT 1) AS musicbrainz,
        EXISTS(SELECT 1 FROM artwork WHERE owner_id = artist.id AND kind = 'Portrait') AS has_portrait
        FROM artists AS artist WHERE artist.id = ?"
    ).bind(id).fetch_optional(&store.0).await?;
    let Some(candidate) = candidate else {
        return Ok(None);
    };
    let mbid = candidate
        .musicbrainz
        .as_deref()
        .and_then(|id| id.parse().ok());
    if candidate.wikidata.is_none() && mbid.is_none() {
        return Ok(None);
    }
    let qid = match candidate.wikidata.filter(|id| valid_id(id)) {
        Some(id) => Some(id),
        None => match mbid {
            Some(mbid) => musicbrainz.wikidata_id(mbid).await?,
            None => None,
        },
    };
    let Some(qid) = qid else {
        return Ok(None);
    };
    let Some(mut response): Option<Response> = source
        .get(&[
            ("action", "wbgetentities"),
            ("format", "json"),
            ("ids", &qid),
            ("props", "descriptions|claims"),
            ("languages", "en"),
        ])
        .await?
    else {
        return Ok(None);
    };
    if let Some(error) = response.error {
        return Err(Error::msg(format!("Wikidata returned {error}")));
    }
    let Some(entity) = response
        .entities
        .remove(&qid)
        .filter(|entity| entity.missing.is_none())
    else {
        return Ok(None);
    };
    let (begin, end) = match candidate.kind.as_str() {
        "Person" => (entity.date("P569"), entity.date("P570")),
        "Group" => (entity.date("P571"), entity.date("P576")),
        _ => (None, None),
    };
    let begin = begin.filter(|_| fields.contains(&"begin_date"));
    let end = end.filter(|_| fields.contains(&"end_date"));
    let description = entity
        .descriptions
        .get("en")
        .filter(|_| fields.contains(&"biography"))
        .map(|text| &text.value);
    let mut db = store.0.begin_with("BEGIN IMMEDIATE").await?;
    let before = crate::editor::snapshot(&mut db, crate::catalog::Kind::Artist, id).await?;
    query("UPDATE artists SET biography = coalesce(biography, ?), begin_date = coalesce(begin_date, ?),
        end_date = coalesce(end_date, ?) WHERE id = ?
        AND ((biography IS NULL AND ? IS NOT NULL) OR (begin_date IS NULL AND ? IS NOT NULL) OR (end_date IS NULL AND ? IS NOT NULL))")
        .bind(description).bind(&begin).bind(&end).bind(id).bind(description).bind(&begin).bind(&end)
        .execute(&mut *db).await?;
    query("INSERT INTO external_ids (owner_id, provider, value) VALUES (?, 'Wikidata', ?) ON CONFLICT DO NOTHING")
        .bind(id).bind(qid).execute(&mut *db).await?;
    for isni in entity
        .values("P213")
        .filter_map(Value::as_str)
        .filter(|_| fields.contains(&"external_ids"))
    {
        let isni = isni.replace(' ', "");
        if isni.len() == 16
            && isni
                .bytes()
                .enumerate()
                .all(|(i, byte)| byte.is_ascii_digit() || (i == 15 && byte == b'X'))
        {
            query("INSERT INTO external_ids (owner_id, provider, value) VALUES (?, 'Isni', ?) ON CONFLICT DO NOTHING")
                .bind(id).bind(isni).execute(&mut *db).await?;
        }
    }
    crate::editor::audit(&mut db, crate::catalog::Kind::Artist, id, &before).await?;
    db.commit().await?;
    Ok((fields.contains(&"artwork") && !candidate.has_portrait)
        .then(|| {
            entity
                .values("P18")
                .find_map(Value::as_str)
                .map(str::to_owned)
        })
        .flatten())
}
