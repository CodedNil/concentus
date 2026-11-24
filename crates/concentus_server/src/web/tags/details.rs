use super::super::{listing::entry_list, record_id};
use crate::{
    catalog::{Entry, Kind},
    store::{Store, store},
};
use concentus_types::{Tag, TagRelation};
use topcoat::{
    Result,
    context::Cx,
    router::{StatusCode, page, query_params},
    view::{BoxView, View, ViewExt, view},
};

#[query_params(error = bad_request)]
struct Filters {
    q: Option<String>,
    kind: Option<String>,
    after: Option<String>,
}

struct Shelf {
    kind: Kind,
    entries: Vec<Entry>,
    next: Option<String>,
}

impl Shelf {
    async fn load(db: &Store, tag: &str, kind: Kind, query: &str, after: &str) -> Result<Self> {
        let mut entries = db
            .entries(kind, query, after, Some((Kind::Tag, tag)), 17)
            .await?;
        let next = if entries.len() == 17 {
            entries.pop();
            entries.last().map(|entry| entry.id.clone())
        } else {
            None
        };
        Ok(Self {
            kind,
            entries,
            next,
        })
    }

    fn render<'a>(&'a self, cx: &'a Cx) -> BoxView<'a> {
        view! {
            <section
                class="tag-shelf"
                data-shelf=(self.kind.path())
                data-next=(self.next.as_deref())
            >
                <header>
                    <h3>
                        (if self.kind == Kind::Tag {
                            "Related tags"
                        } else {
                            self.kind.plural()
                        })
                    </h3>
                    <div class="tag-shelf-nav">
                        <button
                            type="button"
                            data-shelf-step="-1"
                            aria-label="Scroll left"
                        >
                            "←"
                        </button>
                        <button
                            type="button"
                            data-shelf-step="1"
                            aria-label="Scroll right"
                        >
                            "→"
                        </button>
                    </div>
                </header>
                (entry_list(cx, self.kind, &self.entries))
            </section>
        }
        .boxed()
    }
}

#[page("/catalog/tags/{record_id}/details")]
pub(in crate::web) async fn tag_details(cx: &Cx) -> Result<impl View> {
    let params = query_params::<Filters>(cx)?;
    let db = store(cx);
    let id = record_id(cx);
    let tag = db.get::<Tag>(id).await?;
    let mut relation = None;
    let mut shelves = Vec::new();
    if let Some(tag) = &tag {
        if let TagRelation::Parent(parent) | TagRelation::AliasOf(parent) = tag.relation {
            relation = db
                .entries_by_id(Kind::Tag, &[parent.to_string()])
                .await?
                .pop();
        }
        let query: String = params
            .q
            .as_deref()
            .unwrap_or_default()
            .trim()
            .chars()
            .take(160)
            .collect();
        let kind = params.kind.as_deref().and_then(Kind::parse);
        let kinds = kind
            .as_ref()
            .map_or(Kind::ALL.as_slice(), std::slice::from_ref);
        for &kind in kinds {
            let shelf = Shelf::load(
                db,
                id,
                kind,
                &query,
                params.after.as_deref().unwrap_or_default(),
            )
            .await?;
            if !shelf.entries.is_empty() {
                shelves.push(shelf);
            }
        }
    }

    Ok(view! {
        if let Some(tag) = &tag {
            <header class="tag-detail-heading">
                <p class="eyebrow">(format!("{:?}", tag.kind))</p>
                <h2>(&tag.name)</h2>
                if let Some(relation) = &relation {
                    <p>
                        (if matches!(tag.relation, TagRelation::AliasOf(_)) {
                            "Alias of "
                        } else {
                            "Parent: "
                        })
                        <button
                            type="button"
                            class="tag-trigger"
                            data-tag-detail=(&relation.id)
                            aria-haspopup="dialog"
                        >
                            (&relation.title)
                        </button>
                    </p>
                }
            </header>
            <div class="tag-shelves">
                if shelves.is_empty() {
                    <p class="tag-empty">"No matching records."</p>
                }
                for shelf in &shelves {
                    (shelf.render(cx))
                }
            </div>
        } else {
            (StatusCode::NOT_FOUND)
        }
    })
}
