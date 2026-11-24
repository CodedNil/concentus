mod details;
pub(super) use details::tag_details;

use super::{icons::icon, layout::catalog_heading};
use crate::{catalog::Kind, store::store};
use serde::{Deserialize, Serialize};
use sqlx::{SqliteConnection, query, query_as};
use std::collections::HashMap;
use topcoat::{
    Error, Result,
    context::Cx,
    router::{Body, StatusCode, content::Form, request::headers, response::Response, route},
    view::{BoxView, ViewExt, view},
};

const KINDS: [&str; 5] = ["Genre", "Mood", "Activity", "Unclassified", "Hidden"];

#[derive(Clone, Serialize, sqlx::FromRow)]
struct TagRow {
    id: String,
    name: String,
    kind: String,
    parent: Option<String>,
    alias_of: Option<String>,
}

async fn tags(db: &mut SqliteConnection) -> Result<Vec<TagRow>> {
    Ok(query_as(
        "SELECT tags.id, name, kind,
         CASE WHEN relation_kind = 'Parent' THEN related_id END AS parent,
         CASE WHEN relation_kind = 'AliasOf' THEN related_id END AS alias_of
         FROM tags ORDER BY name COLLATE NOCASE, tags.id",
    )
    .fetch_all(db)
    .await?)
}

pub(super) async fn board<'a>(cx: &'a Cx, filter: &str) -> Result<BoxView<'a>> {
    let mut db = store(cx).0.acquire().await?;
    let data = serde_json::to_string(&tags(&mut db).await?)?;
    let filter = filter.to_owned();
    Ok(view! {
        (catalog_heading(cx, "Tags", Some(Kind::Tag), false))
        <section class="tag-workspace" data-tags=(data) data-filter=(filter)>
            <header class="tag-toolbar">
                <p class="tag-help">
                    "Drag tags to move, nest, or merge them. Use the pencil to rename."
                </p>
                <div class="tag-bin-slot"></div>
                <div class="tag-actions">
                    <button type="button" data-tag-edit="" hidden=(true)>
                        "Edit tags"
                    </button>
                    <button type="button" data-tag-undo="" hidden=(true)>"Undo"</button>
                    <button type="button" data-tag-save="" hidden=(true)>
                        "Save changes"
                    </button>
                </div>
                <p class="tag-status" role="status"></p>
            </header>
            <div class="tag-board"></div>
            <template id="tag-row-icon">(icon(cx, Some(Kind::Tag)))</template>
            <template id="tag-pencil-icon">
                <svg viewBox="0 0 24 24" fill="none" aria-hidden="true">
                    <path d="m4 20 4-.8L20 7l-3-3L5 16l-1 4Zm11-14 3 3"></path>
                </svg>
            </template>
            <noscript>
                <p>"Enable JavaScript to browse and edit the tag board."</p>
            </noscript>
        </section>
        <script src="/assets/save-edits.js" defer=(true)></script>
        <script src="/assets/tags.js" defer=(true)></script>
    }
    .boxed())
}

#[derive(Deserialize)]
struct Changes {
    operations: String,
}

#[derive(Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
enum Operation {
    Move {
        id: String,
        kind: String,
        parent: Option<String>,
    },
    Merge {
        id: String,
        target: String,
    },
    Rename {
        id: String,
        name: String,
    },
}

#[route(POST "/catalog/tags/edit")]
pub(super) async fn save(cx: &Cx, Form(form): Form<Changes>) -> Result<Response> {
    if headers(cx)
        .get("sec-fetch-site")
        .is_some_and(|value| value != "same-origin" && value != "none")
    {
        return Ok(Response::builder()
            .status(StatusCode::FORBIDDEN)
            .body("Cross-origin edit rejected".into())?);
    }
    match apply(cx, &form.operations).await {
        Ok(()) => Ok(Response::builder()
            .status(StatusCode::NO_CONTENT)
            .body(Body::default())?),
        Err(error) => Ok(Response::builder()
            .status(StatusCode::BAD_REQUEST)
            .header("content-type", "text/plain; charset=utf-8")
            .body(error.to_string().into())?),
    }
}

async fn apply(cx: &Cx, encoded: &str) -> Result<()> {
    let operations: Vec<Operation> = serde_json::from_str(encoded)?;
    let mut db = store(cx).0.begin_with("BEGIN IMMEDIATE").await?;
    let mut board: HashMap<_, _> = tags(&mut db)
        .await?
        .into_iter()
        .map(|tag| (tag.id.clone(), tag))
        .collect();
    for op in &operations {
        match op {
            Operation::Move { id, kind, parent } => {
                move_tag(&mut db, &mut board, id, kind, parent.as_deref()).await?;
            }
            Operation::Merge { id, target } => merge_tag(&mut db, &mut board, id, target).await?,
            Operation::Rename { id, name } => {
                let name = name.trim();
                if name.is_empty() || name.chars().count() > 160 {
                    return Err(Error::msg("Enter a tag name from 1 to 160 characters."));
                }
                let tag = board
                    .get_mut(id)
                    .ok_or_else(|| Error::msg("Tag no longer exists"))?;
                query("UPDATE tags SET name = ? WHERE id = ?")
                    .bind(name)
                    .bind(id)
                    .execute(&mut *db)
                    .await?;
                name.clone_into(&mut tag.name);
            }
        }
    }
    db.commit().await?;
    tracing::info!(operations = operations.len(), "Tag hierarchy changes saved");
    Ok(())
}

fn entry<'a>(board: &'a HashMap<String, TagRow>, id: &str) -> Result<&'a TagRow> {
    board
        .get(id)
        .ok_or_else(|| Error::msg("A tag has changed. Reload the board and try again."))
}

fn validate_parent(
    board: &HashMap<String, TagRow>,
    id: &str,
    parent: Option<&str>,
    kind: &str,
) -> Result<()> {
    let mut node = parent;
    let mut depth = 1;
    while let Some(key) = node {
        let tag = entry(board, key)?;
        if key == id || tag.alias_of.is_some() || tag.kind != kind || depth >= 3 {
            return Err(Error::msg(
                "Tags can nest up to three levels within a single list.",
            ));
        }
        depth += 1;
        node = tag.parent.as_deref();
    }
    let mut descendants = vec![(id, depth)];
    while let Some((key, depth)) = descendants.pop() {
        if depth > 3 {
            return Err(Error::msg(
                "Tags can nest up to three levels within a single list.",
            ));
        }
        descendants.extend(
            board
                .values()
                .filter(|tag| tag.parent.as_deref() == Some(key))
                .map(|tag| (tag.id.as_str(), depth + 1)),
        );
    }
    Ok(())
}

async fn move_tag(
    db: &mut SqliteConnection,
    board: &mut HashMap<String, TagRow>,
    id: &str,
    kind: &str,
    parent: Option<&str>,
) -> Result<()> {
    entry(board, id)?;
    if !KINDS.contains(&kind) {
        return Err(Error::msg("Unknown tag kind"));
    }
    validate_parent(board, id, parent, kind)?;
    query("UPDATE tags SET related_id = ?, relation_kind = ? WHERE id = ?")
        .bind(parent)
        .bind(parent.map(|_| "Parent"))
        .bind(id)
        .execute(&mut *db)
        .await?;
    let mut subtree = vec![id.to_owned()];
    while let Some(key) = subtree.pop() {
        for tag in board.values_mut() {
            if tag.id == key {
                kind.clone_into(&mut tag.kind);
                if tag.id == id {
                    tag.parent = parent.map(str::to_owned);
                    tag.alias_of = None;
                }
            } else if tag.parent.as_deref() == Some(&key) {
                subtree.push(tag.id.clone());
            } else if tag.alias_of.as_deref() == Some(&key) {
                kind.clone_into(&mut tag.kind);
            }
        }
        query("UPDATE tags SET kind = ? WHERE id = ? OR (related_id = ? AND relation_kind = 'AliasOf')")
            .bind(kind)
            .bind(&key)
            .bind(key)
            .execute(&mut *db)
            .await?;
    }
    Ok(())
}

async fn merge_tag(
    db: &mut SqliteConnection,
    board: &mut HashMap<String, TagRow>,
    id: &str,
    target: &str,
) -> Result<()> {
    entry(board, id)?;
    let destination = entry(board, target)?;
    if destination.alias_of.is_some() {
        return Err(Error::msg("Choose a canonical tag to merge into."));
    }
    let kind = destination.kind.clone();
    let mut ancestor = Some(target);
    for _ in 0..3 {
        let Some(key) = ancestor else { break };
        if key == id {
            return Err(Error::msg(
                "A tag cannot merge into itself or one of its children.",
            ));
        }
        ancestor = entry(board, key)?.parent.as_deref();
    }
    if ancestor.is_some() {
        return Err(Error::msg("The destination exceeds the nesting limit."));
    }
    let children: Vec<_> = board
        .values()
        .filter(|tag| tag.parent.as_deref() == Some(id))
        .map(|tag| tag.id.clone())
        .collect();
    for child in children {
        move_tag(db, board, &child, &kind, Some(target)).await?;
    }
    query(
        "INSERT INTO record_tags (owner_id, tag_id, weight)
         SELECT owner_id, ?, weight FROM record_tags WHERE tag_id = ?
         ON CONFLICT (owner_id, tag_id) DO UPDATE SET weight = max(record_tags.weight, excluded.weight)",
    )
    .bind(target)
    .bind(id)
    .execute(&mut *db)
    .await?;
    query("DELETE FROM record_tags WHERE tag_id = ?")
        .bind(id)
        .execute(&mut *db)
        .await?;
    query(
        "UPDATE tags SET related_id = ?, relation_kind = 'AliasOf', kind = ?
           WHERE id = ? OR (related_id = ? AND relation_kind = 'AliasOf')",
    )
    .bind(target)
    .bind(&kind)
    .bind(id)
    .bind(id)
    .execute(db)
    .await?;
    for tag in board
        .values_mut()
        .filter(|tag| tag.id == id || tag.alias_of.as_deref() == Some(id))
    {
        tag.alias_of = Some(target.to_owned());
        tag.parent = None;
        tag.kind.clone_from(&kind);
    }
    Ok(())
}
