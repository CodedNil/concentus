use super::{
    icons::icon,
    layout::{catalog_heading, layout},
    listing::entry_list,
};
use crate::{
    catalog::{Entry, Kind},
    enrichment::{Enrichment, Update},
    store::{Store, store},
};
use topcoat::{
    Result,
    context::{Cx, context},
    router::{page, query_params},
    view::{BoxView, View, ViewExt, emit, live, view},
};

#[query_params(error = bad_request)]
struct SearchParams {
    q: Option<String>,
    kind: Option<String>,
    after: Option<String>,
    sort: Option<String>,
}

struct Section {
    kind: Kind,
    entries: Vec<Entry>,
    next: Option<String>,
    popular: bool,
}

impl Section {
    async fn load(db: &Store, kind: Kind, query: &str, after: &str, popular: bool) -> Result<Self> {
        const LIMIT: u32 = 24;
        let mut entries = if popular {
            db.popular_entries(kind, after, LIMIT + 1).await?
        } else {
            db.entries(kind, query, after, None, LIMIT + 1).await?
        };
        let next = if entries.len() > LIMIT as usize {
            entries.pop();
            entries.last().map(|entry| entry.id.clone())
        } else {
            None
        };
        Ok(Self {
            kind,
            entries,
            next,
            popular,
        })
    }
}

#[page("/catalog/more")]
pub(super) async fn more(cx: &Cx) -> Result<impl View> {
    let params = query_params::<SearchParams>(cx)?;
    let section = match (
        params.kind.as_deref().and_then(Kind::parse),
        params.after.as_deref(),
    ) {
        (Some(kind), Some(after)) => {
            let query = search_input(params.q.as_deref());
            Some(
                Section::load(
                    store(cx),
                    kind,
                    &query,
                    after,
                    popular_order(params, Some(kind), &query),
                )
                .await?,
            )
        }
        _ => None,
    };
    Ok(view! {
        if let Some(section) = &section {
            <div data-catalog-batch="" data-next=(section.next.as_deref())>
                (entry_list(cx, section.kind, &section.entries))
            </div>
        } else {
            (topcoat::router::StatusCode::BAD_REQUEST)
        }
    })
}

fn popular_order(params: &SearchParams, kind: Option<Kind>, query: &str) -> bool {
    params.sort.as_deref() == Some("popular")
        && query.is_empty()
        && kind.is_some_and(|kind| Kind::MUSIC.contains(&kind))
}

fn search_input(query: Option<&str>) -> String {
    query.unwrap_or_default().trim().chars().take(160).collect()
}

enum SearchStatus {
    Local,
    Searching,
    Finished { imported: bool, failed: bool },
}

impl SearchStatus {
    const fn notice(&self) -> Option<&'static str> {
        match self {
            Self::Searching => Some("Searching MusicBrainz…"),
            Self::Finished {
                imported: true,
                failed: true,
            } => Some("MusicBrainz results saved. Some collections are temporarily unavailable."),
            Self::Finished {
                imported: false,
                failed: true,
            } => Some("MusicBrainz is temporarily unavailable. Try your search again shortly."),
            _ => None,
        }
    }
}

#[page("/")]
pub(super) async fn home(cx: &Cx) -> Result<impl View> {
    let params = query_params::<SearchParams>(cx)?;
    let selected = params.kind.as_deref().and_then(Kind::parse);
    let query = search_input(params.q.as_deref());
    if selected == Some(Kind::Tag) {
        return Ok(layout(
            cx,
            "Tags".into(),
            selected,
            query.clone(),
            super::tags::board(cx, &query).await?,
        ));
    }
    let after = params.after.as_deref().unwrap_or_default();
    let db = store(cx);
    let mut sections = Vec::new();
    let kinds = selected
        .as_ref()
        .map_or(Kind::ALL.as_slice(), std::slice::from_ref);
    let overview = selected.is_none() && query.is_empty() && after.is_empty();
    let popular = popular_order(params, selected, &query);
    for &kind in kinds {
        let section = if overview {
            Section {
                kind,
                entries: db.recent_entries(kind, 8).await?,
                next: None,
                popular: false,
            }
        } else {
            Section::load(db, kind, &query, after, popular).await?
        };
        sections.push(section);
    }
    if !query.is_empty() {
        tracing::info!(
            collection = selected.map_or("all", Kind::path),
            results = sections
                .iter()
                .map(|section| section.entries.len())
                .sum::<usize>(),
            "Local catalog search completed"
        );
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
    Ok(layout(
        cx,
        heading,
        selected,
        search,
        view! {
            (catalog_heading(
                cx,
                &title,
                selected,
                selected.is_none() && query.is_empty(),
            ))
            if selected.is_none() && query.is_empty() {
                <nav class="collection-links" aria-label="Browse music">
                    for kind in [Kind::Artist, Kind::Album, Kind::Track] {
                        <a
                            href=(format!("/?kind={}", kind.path()))
                            data-kind=(kind.path())
                        >
                            <span class="collection-icon">(icon(cx, Some(kind)))</span>
                            <span>(kind.plural())</span>
                            <span class="arrow" aria-hidden="true">"↗"</span>
                        </a>
                    }
                </nav>
            }
            if let Some(kind) = selected && query.is_empty() {
                <nav class="catalog-sort" aria-label="Sort collection">
                    <a
                        href=(format!("/?kind={}", kind.path()))
                        aria-current=(if popular { "false" } else { "page" })
                    >
                        "Catalog order"
                    </a>
                    <a
                        href=(format!("/?kind={}&sort=popular", kind.path()))
                        aria-current=(if popular { "page" } else { "false" })
                    >
                        "Popular this week"
                    </a>
                </nav>
            }
            if overview && sections.iter().any(|section| !section.entries.is_empty()) {
                (overview_sections(cx, &sections))
            } else if upstream.is_empty() {
                (search_results(cx, &sections, &query, SearchStatus::Local))
                if sections.iter().any(|section| section.next.is_some()) {
                    <script src="/assets/browse.js" defer=(true)></script>
                }
            } else {
                (live! {
                    let mut sections = sections;
                    let enrichment: &Enrichment = context(cx);
                    let mut updates = enrichment.search(
                        db.clone(),
                        upstream,
                        query.clone(),
                    );
                    let mut failed = false;
                    loop {
                        emit! {
                            (search_results(
                                cx,
                                &sections,
                                &query,
                                SearchStatus::Searching,
                            ))
                        }?;
                        let Some(update) = updates.recv().await else {
                            break
                        };
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
                    }
                    let imported = sections
                        .iter()
                        .any(|section| !section.entries.is_empty());
                    emit! {
                        (search_results(
                            cx,
                            &sections,
                            &query,
                            SearchStatus::Finished { imported, failed },
                        ))
                    }
                })
            }
        }
        .boxed(),
    ))
}

fn overview_sections<'a>(cx: &'a Cx, sections: &'a [Section]) -> BoxView<'a> {
    view! {
        for section in sections.iter().filter(|section| !section.entries.is_empty()) {
            <section class="panel overview-collection">
                <div class="panel-heading">
                    <h2>(section.kind.plural())</h2>
                    <a href=(format!("/?kind={}", section.kind.path()))>
                        "View all "
                        (section.kind.plural().to_lowercase())
                        " →"
                    </a>
                </div>
                (entry_list(cx, section.kind, &section.entries))
            </section>
        }
    }
    .boxed()
}

fn search_results<'a>(
    cx: &'a Cx,
    sections: &'a [Section],
    query: &'a str,
    status: SearchStatus,
) -> BoxView<'a> {
    let has_entries = sections.iter().any(|section| !section.entries.is_empty());
    view! {
        if let Some(notice) = status.notice() {
            <p class="notice" role="status">(notice)</p>
        }
        if !has_entries && !matches!(status, SearchStatus::Searching) {
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
                        <form action="/" method="get" class="catalog-continuation">
                            <input type="hidden" name="q" value=(query)>
                            <input
                                type="hidden"
                                name="kind"
                                value=(section.kind.path())
                            >
                            <input type="hidden" name="after" value=(next)>
                            if section.popular {
                                <input type="hidden" name="sort" value="popular">
                            }
                            <button type="submit">
                                "More "
                                (section.kind.plural())
                                " →"
                            </button>
                            <span role="status"></span>
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
