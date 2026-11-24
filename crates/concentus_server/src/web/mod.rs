mod artwork;
mod detail;
mod editor;

use crate::{
    catalog::{Entry, Kind},
    enrichment::{Enrichment, Update},
    store::store,
};
use topcoat::{
    Result,
    context::{Cx, context},
    router::{
        Body, RouterBuilder, StatusCode, page, path_param, query_params, response::Response, route,
    },
    view::{BoxView, View, ViewExt, emit, live, view},
};

pub fn register(builder: RouterBuilder) -> RouterBuilder {
    builder
        .route(stylesheet)
        .page(home)
        .page(record)
        .page(editor::edit_page)
        .route(editor::edit_action)
}

#[route(GET "/assets/catalog.css")]
async fn stylesheet(cx: &Cx) -> Result<Response> {
    let _ = cx;
    Ok(Response::builder()
        .header("content-type", "text/css; charset=utf-8")
        .body(Body::from(include_str!("catalog.css")))?)
}

#[query_params(error = bad_request)]
struct SearchParams {
    q: Option<String>,
    kind: Option<String>,
    after: Option<String>,
}

struct Section {
    kind: Kind,
    entries: Vec<Entry>,
    next: Option<String>,
}

#[page("/")]
async fn home(cx: &Cx) -> Result<impl View> {
    let params = query_params::<SearchParams>(cx)?;
    let selected = params.kind.as_deref().and_then(Kind::parse);
    let input = params.q.as_deref().unwrap_or_default().trim();
    let query: String = input.chars().take(160).collect();
    let after = params.after.as_deref().unwrap_or_default();
    let db = store(cx);
    let mut sections = Vec::new();
    let kinds = selected
        .as_ref()
        .map_or(Kind::ALL.as_slice(), std::slice::from_ref);
    let page_size: u32 = if selected.is_some() || !query.is_empty() {
        24
    } else {
        8
    };
    for &kind in kinds {
        let mut entries = db.entries(kind, &query, after, None, page_size + 1).await?;
        let next = if entries.len() > page_size as usize {
            entries.pop();
            entries.last().map(|entry| entry.id.clone())
        } else {
            None
        };
        sections.push(Section {
            kind,
            entries,
            next,
        });
    }
    let upstream = if !query.is_empty()
        && after.is_empty()
        && sections.iter().all(|section| section.entries.is_empty())
    {
        kinds
            .iter()
            .copied()
            .filter(|kind| Kind::MUSIC.contains(kind))
            .collect::<Vec<_>>()
    } else {
        Vec::new()
    };
    let title = if query.is_empty() {
        selected.map_or("Music catalog", Kind::plural).to_owned()
    } else {
        format!("Results for “{query}”")
    };
    let heading = title.clone();
    let search = query.clone();
    Ok(layout(cx, heading, selected, search, view! {
        <section class="page-heading">
            if query.is_empty() {
                <p class="eyebrow">"Your library"</p>
            }
            <h1>(&title)</h1>
        </section>
        if upstream.is_empty() {
            (search_results(cx, &sections, &query, None, false))
        } else {
            (live! {
                let mut sections = sections;
                let enrichment: &Enrichment = context(cx);
                let mut updates = enrichment.search(db.clone(), upstream, query.clone());
                emit! {
                    (search_results(
                        cx,
                        &sections,
                        &query,
                        Some("Searching MusicBrainz…"),
                        true,
                    ))
                }?;
                let mut failed = false;
                while let Some(update) = updates.recv().await {
                    match update {
                        Update::Unavailable => failed = true,
                        Update::Results(kind, ids) => {
                            if let Some(section) = sections
                                .iter_mut()
                                .find(|section| section.kind == kind) {
                                let entries = db.entries_by_id(kind, &ids).await?;
                                section.entries = ranked_entries(entries, &ids);
                            }
                        }
                    }
                    emit! {
                        (search_results(
                            cx,
                            &sections,
                            &query,
                            Some("Searching MusicBrainz…"),
                            true,
                        ))
                    }?;
                }
                let imported = sections
                    .iter()
                    .any(|section| !section.entries.is_empty());
                let notice = match (imported, failed) {
                    (true, true) => Some(
                        "MusicBrainz results saved. Some collections are temporarily unavailable.",
                    ),
                    (true, false) => Some("MusicBrainz results saved to your catalog."),
                    (false, true) => Some(
                        "MusicBrainz is temporarily unavailable. Try your search again shortly.",
                    ),
                    (false, false) => None,
                };
                emit! { (search_results(cx, &sections, &query, notice, false)) }
            })
        }
    }.boxed()))
}

fn search_results<'a>(
    cx: &'a Cx,
    sections: &'a [Section],
    query: &'a str,
    notice: Option<&'static str>,
    searching: bool,
) -> BoxView<'a> {
    let has_entries = sections.iter().any(|section| !section.entries.is_empty());
    view! {
        if let Some(notice) = notice {
            <p class="notice" role="status">(notice)</p>
        }
        if !has_entries && !searching {
            <div class="empty">
                if query.is_empty() {
                    <h2>"Nothing here yet"</h2>
                    <p>
                        "Search for an artist, album or track to bring it into your catalog."
                    </p>
                } else {
                    <h2>"No matches"</h2>
                    <p>
                        "Try a different name, or enter an artist and title separated by a dash."
                    </p>
                }
            </div>
        }
        for section in sections {
            if !section.entries.is_empty() {
                <section class="panel">
                    <div class="panel-heading"><h2>(section.kind.plural())</h2></div>
                    (entry_list(cx, section.kind, &section.entries))
                    if let Some(next) = &section.next {
                        <form action="/" method="get" class="pagination">
                            <input type="hidden" name="q" value=(query)>
                            <input
                                type="hidden"
                                name="kind"
                                value=(section.kind.path())
                            >
                            <input type="hidden" name="after" value=(next)>
                            <button type="submit">
                                "More "
                                (section.kind.plural())
                                " →"
                            </button>
                        </form>
                    }
                </section>
            }
        }
    }
    .boxed()
}

fn ranked_entries(entries: Vec<Entry>, ids: &[String]) -> Vec<Entry> {
    let mut by_id: std::collections::HashMap<_, _> = entries
        .into_iter()
        .map(|entry| (entry.id.clone(), entry))
        .collect();
    ids.iter().filter_map(|id| by_id.remove(id)).collect()
}

path_param!(collection);
path_param!(record_id);

#[page("/catalog/{collection}/{record_id}")]
async fn record(cx: &Cx) -> Result<impl View> {
    let id = record_id(cx);
    let kind = Kind::parse(collection(cx));
    let db = store(cx);
    let record = if let Some(kind) = kind {
        db.detail(kind, id).await?
    } else {
        None
    };
    let (title, page) = detail::Page::load(db, record).await?;
    let status = if page.record.is_some() {
        StatusCode::OK
    } else {
        StatusCode::NOT_FOUND
    };
    Ok(layout(
        cx,
        title,
        kind,
        String::new(),
        view! {
            (status)
            (detail::render(cx, &page))
        }
        .boxed(),
    ))
}

fn layout<'a>(
    cx: &'a Cx,
    title: String,
    selected: Option<Kind>,
    query: String,
    content: BoxView<'a>,
) -> impl View + 'a {
    view! {
        <!DOCTYPE html>
        <html lang="en">
            <head>
                <meta charset="utf-8">
                <meta name="viewport" content="width=device-width, initial-scale=1">
                <title>
                    (title.clone())
                    " · Concentus"
                </title>
                <link rel="stylesheet" href="/assets/catalog.css">
            </head>
            <body>
                <a class="skip" href="#main">"Skip to content"</a>
                <header class="topbar">
                    <div class="shell bar">
                        <a class="brand" href="/">
                            <span class="mark" aria-hidden="true">"◉"</span>
                            "Concentus"
                        </a>
                        <form
                            class="search"
                            action="/"
                            method="get"
                            role="search"
                            onsubmit="this.querySelector('button').textContent = 'Searching…'"
                        >
                            <label class="sr-only" for="query">
                                "Search the catalog"
                            </label>
                            <input
                                id="query"
                                type="search"
                                name="q"
                                value=(query.clone())
                                placeholder="Search catalog"
                                maxlength="160"
                                required=(true)
                            >
                            <label class="sr-only" for="category">"Collection"</label>
                            <select id="category" name="kind">
                                <option value="">"All types"</option>
                                for kind in Kind::ALL {
                                    <option
                                        value=(kind.path())
                                        selected=(selected == Some(kind))
                                    >
                                        (kind.plural())
                                    </option>
                                }
                            </select>
                            <button type="submit">"Search"</button>
                        </form>
                    </div>
                </header>
                <div class="shell">
                    <nav class="tabs" aria-label="Collections">
                        <a href="/">"Overview"</a>
                        for kind in Kind::ALL {
                            <a
                                href=(format!("/?kind={}", kind.path()))
                                aria-current=(if selected == Some(kind) {
                                    "page"
                                } else {
                                    "false"
                                })
                            >
                                (kind.plural())
                            </a>
                        }
                    </nav>
                    <main id="main">(content)</main>
                    <footer>
                        "Catalog enrichment from "
                        <a href="https://musicbrainz.org/">"MusicBrainz"</a>
                    </footer>
                </div>
            </body>
        </html>
    }
}

fn entry_list<'a>(cx: &'a Cx, kind: Kind, entries: &'a [Entry]) -> BoxView<'a> {
    view! {
        <ul class="entries">
            for entry in entries {
                <li>
                    <a
                        class="entry has-tip"
                        href=(kind.href(&entry.id))
                        title=(entry.preview.as_deref())
                    >
                        <span class="entry-icon" aria-hidden="true">
                            (icon(kind))
                            (artwork::image(cx, artwork::candidates(entry)))
                        </span>
                        <span>
                            <strong>(&entry.title)</strong>
                            if let Some(subtitle) = &entry.subtitle {
                                <small>(subtitle)</small>
                            }
                        </span>
                        <span class="arrow" aria-hidden="true">"↗"</span>
                        if let Some(preview) = &entry.preview {
                            <span class="tip" aria-hidden="true">
                                <strong>(&entry.title)</strong>
                                <small>(preview)</small>
                            </span>
                        }
                    </a>
                </li>
            }
        </ul>
    }
    .boxed()
}

const fn icon(kind: Kind) -> &'static str {
    match kind {
        Kind::Artist => "◍",
        Kind::Album => "◎",
        Kind::Track => "♫",
        Kind::Tag => "#",
    }
}
