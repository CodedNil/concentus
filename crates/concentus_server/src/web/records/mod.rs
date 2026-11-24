mod album;
mod artist;
mod metadata;
mod track;

use super::{
    artwork, collection, editor::InlineEditor, icons::icon, layout::layout, listing::entry_list,
    record_id,
};
use crate::{
    catalog::{Detail, Entry, Kind},
    store::{Store, store},
};
use concentus_types::{ArtistKind, Credit};
use std::collections::HashMap;
use topcoat::{
    Result,
    context::Cx,
    router::{StatusCode, page, query_params},
    view::{BoxView, View, ViewExt, view},
};

use metadata::{billed, related_link};

#[query_params(error = bad_request)]
struct RecordParams {
    edit: Option<bool>,
}

#[page("/catalog/{collection}/{record_id}")]
pub(super) async fn record_page(cx: &Cx) -> Result<impl View> {
    let id = record_id(cx);
    let kind = Kind::parse(collection(cx)).filter(|kind| Kind::MUSIC.contains(kind));
    let db = store(cx);
    let detail = if let Some(kind) = kind {
        db.detail(kind, id).await?
    } else {
        None
    };
    let Some(detail) = detail else {
        return Ok(layout(
            cx,
            "Record not found".into(),
            kind,
            String::new(),
            view! {
                (StatusCode::NOT_FOUND)
                <section class="empty">
                    <h1>"Record not found"</h1>
                    <p>"This record is unavailable in the local catalog."</p>
                    <a href="/">"Return to the catalog"</a>
                </section>
            }
            .boxed(),
        ));
    };
    let mut page = Page::load(db, detail).await?;
    let params = query_params::<RecordParams>(cx)?;
    if params.edit == Some(true) && Kind::MUSIC.contains(&page.kind) {
        page.editor = Some(InlineEditor::load(db, page.kind, &page.id).await?);
    }
    Ok(layout(
        cx,
        page.title.clone(),
        Some(page.kind),
        String::new(),
        view! { (page.render(cx)) }.boxed(),
    ))
}

struct Page {
    record: Detail,
    id: String,
    title: String,
    kind: Kind,
    names: HashMap<String, Entry>,
    related: Vec<(Kind, Vec<Entry>)>,
    images: Vec<String>,
    editor: Option<InlineEditor>,
    albums_pending: bool,
}

impl Page {
    async fn load(db: &Store, record: Detail) -> Result<Self> {
        let mut references = Vec::new();
        let mut artist_credits = |credits: &[Credit]| {
            references.extend(
                credits
                    .iter()
                    .map(|credit| (Kind::Artist, credit.artist_id.to_string())),
            );
        };
        let (kind, id, title) = match &record {
            Detail::Artist(artist) => {
                if let ArtistKind::Group { members } = &artist.kind {
                    references.extend(
                        members
                            .iter()
                            .map(|member| (Kind::Artist, member.artist_id.to_string())),
                    );
                }
                (Kind::Artist, artist.id.to_string(), artist.name.clone())
            }
            Detail::Album(album) => {
                artist_credits(&album.credits);
                references.extend(album.track_ids().map(|id| (Kind::Track, id.to_string())));
                if let Some(base) = album.edition.as_ref().and_then(|edition| edition.of) {
                    references.push((Kind::Album, base.to_string()));
                }
                (Kind::Album, album.id.to_string(), album.display_title())
            }
            Detail::Track(track) => {
                artist_credits(&track.credits);
                if let Some(album) = track.album_id() {
                    references.push((Kind::Album, album.to_string()));
                }
                references.extend(
                    track
                        .derived_from
                        .keys()
                        .map(|id| (Kind::Track, id.to_string())),
                );
                for lyrics in track.lyrics.values() {
                    references.extend(
                        lyrics
                            .channels
                            .iter()
                            .filter_map(|channel| channel.artist_id)
                            .map(|id| (Kind::Artist, id.to_string())),
                    );
                }
                (Kind::Track, track.id.to_string(), track.title.clone())
            }
        };
        let mut names = HashMap::new();
        for target in Kind::ALL {
            let ids = references
                .iter()
                .filter(|(kind, _)| *kind == target)
                .map(|(_, id)| id.clone())
                .collect::<Vec<_>>();
            for entry in db.entries_by_id(target, &ids).await? {
                names.insert(target.href(&entry.id), entry);
            }
        }
        let mut related = Vec::new();
        let targets = match &record {
            Detail::Artist(_) => &[Kind::Album, Kind::Track, Kind::Artist][..],
            Detail::Track(_) => &[Kind::Album][..],
            Detail::Album(_) => &[],
        };
        for &target in targets {
            let entries = db.entries(target, "", "", Some((kind, &id)), 50).await?;
            if !entries.is_empty() || (kind == Kind::Artist && target == Kind::Album) {
                related.push((target, entries));
            }
        }
        let albums_pending = if kind == Kind::Artist {
            sqlx::query_scalar(
                "SELECT EXISTS(SELECT 1 FROM enrichment_needed WHERE record_id = ? AND field = 'discography')",
            )
            .bind(&id)
            .fetch_one(&db.0)
            .await?
        } else {
            false
        };
        let images = db
            .entries_by_id(kind, std::slice::from_ref(&id))
            .await?
            .first()
            .map_or_else(Vec::new, artwork::candidates);
        Ok(Self {
            record,
            id,
            title,
            kind,
            names,
            related,
            images,
            editor: None,
            albums_pending,
        })
    }
}

impl Page {
    fn edit_fields<'a>(&'a self, cx: &'a Cx, keys: &'a [&'a str]) -> BoxView<'a> {
        view! {
            if let Some(editor) = &self.editor {
                (editor.fields(cx, keys))
            }
        }
        .boxed()
    }

    fn render<'a>(&'a self, cx: &'a Cx) -> BoxView<'a> {
        let record = &self.record;
        view! {
            <div class="record-bar">
                <nav class="crumbs" aria-label="Breadcrumb">
                    <a href="/">"Catalog"</a>
                    <span aria-hidden="true">"/"</span>
                    <a href=(format!("/?kind={}", self.kind.path()))>
                        (self.kind.plural())
                    </a>
                </nav>
                if Kind::MUSIC.contains(&self.kind) {
                    if self.editor.is_some() {
                        <a class="edit-link" href=(self.kind.href(&self.id))>
                            "Done editing"
                        </a>
                    } else {
                        <a
                            class="edit-link"
                            href=(format!("{}?edit=true", self.kind.href(&self.id)))
                        >
                            "Edit metadata"
                        </a>
                    }
                }
            </div>
            <div class="detail" data-kind=(self.kind.path())>
                (heading(cx, self, record))
                if let Some(editor) = &self.editor {
                    (editor.controls(cx))
                    (self.edit_fields(
                        cx,
                        match self.kind {
                            Kind::Artist => &["name", "origin", "biography"],
                            Kind::Album => &["title", "description"],
                            Kind::Track => &["title", "album_id"],
                            Kind::Tag => &[],
                        },
                    ))
                }
                match record {
                    Detail::Artist(artist) => {
                        (artist::render(cx, self, artist))
                    }
                    Detail::Album(album) => {
                        (album::render(cx, self, album))
                    }
                    Detail::Track(track) => {
                        (track::render(cx, self, track))
                    }
                }
                for (kind, entries) in &self.related {
                    let discography = self.kind == Kind::Artist && *kind == Kind::Album;
                    <section class="panel" id=(discography.then_some("artist-albums"))>
                        <div class="panel-heading">
                            <h2>
                                (match (self.kind, *kind) {
                                    (Kind::Track, Kind::Album) => "Appears on",
                                    (Kind::Artist, Kind::Artist) => "Groups",
                                    _ => kind.plural(),
                                })
                            </h2>
                            if discography {
                                if self.albums_pending {
                                    <small role="status">"Album discovery queued"</small>
                                } else {
                                    <form
                                        method="post"
                                        action=(format!("{}/edit", self.kind.href(&self.id)))
                                    >
                                        <button
                                            class="fetch-albums"
                                            name="action"
                                            value="discography"
                                        >
                                            "Find albums"
                                        </button>
                                    </form>
                                }
                            }
                        </div>
                        if entries.is_empty() {
                            <p class="panel-copy">"No albums in the catalog yet."</p>
                        } else {
                            (entry_list(cx, *kind, entries))
                        }
                    </section>
                }
            </div>
        }
        .boxed()
    }
}

fn heading<'a>(cx: &'a Cx, page: &'a Page, record: &'a Detail) -> BoxView<'a> {
    view! {
        <section class="record-heading" data-kind=(page.kind.path())>
            <div class="heading-art">(icon(cx, Some(page.kind)))</div>
            <div class="cover" aria-hidden="true">
                <div class="art-face">
                    (icon(cx, Some(page.kind)))
                    (artwork::image(cx, page.images.clone()))
                </div>
            </div>
            <div class="heading-copy">
                <p class="eyebrow">(page.kind.label())</p>
                <h1>(&page.title)</h1>
                match record {
                    Detail::Artist(artist) => {
                        if let Some(origin) = &artist.origin {
                            <p>(origin)</p>
                        }
                        if let Some(biography) = &artist.biography {
                            <p class="prose">(biography)</p>
                        }
                    }
                    Detail::Album(album) => {
                        (billed(cx, page, &album.credits))
                        if let Some(description) = &album.description {
                            <p class="prose">(description)</p>
                        }
                    }
                    Detail::Track(track) => {
                        (billed(cx, page, &track.credits))
                        if let Some(album) = track.album_id() {
                            <p class="album-line">
                                (related_link(cx, page, Kind::Album, &album.to_string()))
                            </p>
                        }
                    }
                }
            </div>
        </section>
    }
    .boxed()
}
