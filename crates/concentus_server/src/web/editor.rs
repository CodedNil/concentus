use super::{collection, record_id};
use crate::{
    catalog::Kind,
    editor::{EditBatch, EnrichmentState, Revision, Snapshot},
    enrichment::{Enrichment, can_refresh},
    store::{Store, store},
};
use serde::Deserialize;
use serde_json::Value;
use topcoat::{
    Result,
    context::{Cx, context},
    router::{Body, StatusCode, content::Form, request::headers, response::Response, route},
    view::{BoxView, ViewExt, view},
};

#[derive(Deserialize)]
struct EditForm {
    action: String,
    value: Option<String>,
}

pub(super) struct InlineEditor {
    kind: Kind,
    action: String,
    fields: Snapshot,
    revisions: Vec<Revision>,
    pending: Vec<EnrichmentState>,
}

impl InlineEditor {
    pub(super) async fn load(db: &Store, kind: Kind, id: &str) -> Result<Self> {
        let (fields, revisions, pending) = db.editor(kind, id).await?;
        Ok(Self {
            kind,
            action: format!("{}/edit", kind.href(id)),
            fields,
            revisions,
            pending,
        })
    }

    pub(super) fn controls<'a>(&'a self, cx: &'a Cx) -> BoxView<'a> {
        view! {
            <section class="panel edit-toolbar">
                <div class="edit-actions">
                    <form id="save-record" method="post" action=(&self.action)>
                        <button type="submit" disabled=(true)>"Save edits"</button>
                    </form>
                    if !self.revisions.is_empty() {
                        (history_button(cx, "record-history", "Record history"))
                    }
                    <span id="edit-status" role="status"></span>
                </div>
                if !self.revisions.is_empty() {
                    <div class="edit-history" id="record-history" hidden=(true)>
                        <h3>"Record history"</h3>
                        for minute in revision_minutes(&self.revisions) {
                            <button
                                type="button"
                                data-restore-record=(minute.to_string())
                            >
                                "Use values before "
                                (minute_label(minute))
                            </button>
                        }
                    </div>
                }
            </section>
            <script src="/assets/save-edits.js" defer=(true)></script>
            <script src="/assets/editor.js" defer=(true)></script>
        }
        .boxed()
    }

    pub(super) fn fields<'a>(&'a self, cx: &'a Cx, keys: &'a [&'a str]) -> BoxView<'a> {
        view! {
            <div class="inline-fields">
                for &field in keys {
                    if let Some(value) = self.fields.get(field) {
                        (self.field_editor(cx, field, value))
                    }
                }
            </div>
        }
        .boxed()
    }

    fn field_editor<'a>(&'a self, cx: &'a Cx, field: &'a str, value: &'a Value) -> BoxView<'a> {
        let numeric = value.is_number()
            || matches!(
                field,
                "tempo_bpm"
                    | "time_signature"
                    | "loudness_lufs"
                    | "peak"
                    | "acousticness"
                    | "danceability"
                    | "energy"
                    | "instrumentalness"
                    | "liveness"
                    | "speechiness"
                    | "valence"
            );
        let structured = value.is_object() || value.is_array();
        let required = matches!(field, "name" | "title" | "kind" | "duration_ms");
        let multiline = matches!(field, "about" | "biography" | "description");
        let label = field.replace('_', " ");
        let input_id = format!("edit-{field}");
        let history_id = format!("history-{field}");
        let revisions = self
            .revisions
            .iter()
            .filter(move |entry| entry.field == field);
        let has_history = revisions.clone().next().is_some();
        let format = if structured {
            "json"
        } else if numeric {
            "number"
        } else if required {
            "text"
        } else {
            "optional-text"
        };
        let displayed = if structured || numeric && !value.is_null() {
            serde_json::to_string_pretty(value).unwrap_or_default()
        } else {
            value.as_str().unwrap_or_default().to_owned()
        };
        view! {
            <div
                class="inline-field"
                data-field=(field)
                data-original=(value.to_string())
                data-format=(format)
                data-wide=(if structured || multiline { "true" } else { "false" })
            >
                <div class="field-header">
                    <label for=(&input_id)>(&label)</label>
                    if let Some(state) = self
                        .pending
                        .iter()
                        .find(|state| state.field == field) {
                        <small title=(state.error.as_deref())>
                            (&state.status)
                            if let Some(provider) = &state.provider {
                                " · "
                                (provider)
                            }
                            if let Some(checked) = state.checked_at.and_then(
                                |second| jiff::Timestamp::from_second(second).ok(),
                            ) {
                                " · "
                                (checked.to_string())
                            }
                        </small>
                    }
                    if has_history {
                        (history_button(cx, &history_id, &format!("History for {label}")))
                    }
                    if can_refresh(self.kind, field) {
                        <label
                            class="refresh-toggle"
                            title="Queue a refresh for this field when you save. The field is locked while selected."
                        >
                            <input
                                type="checkbox"
                                data-refresh=""
                                aria-label=(format!("Refresh {label} on save"))
                            >
                            <svg viewBox="0 0 24 24" aria-hidden="true">
                                <path d="M20 11a8 8 0 1 0-2 6m2-11v5h-5"></path>
                            </svg>
                        </label>
                    }
                </div>
                if field == "lyrics" {
                    <div class="lyrics-editor">
                        <label>
                            "Language"
                            <input type="text" data-language="" placeholder="en">
                        </label>
                        <label>
                            "Lyrics"
                            <textarea
                                rows="3"
                                data-lyrics=""
                                placeholder="Leave blank to remove this language"
                            ></textarea>
                        </label>
                    </div>
                    <label class="structured-label" for=(&input_id)>
                        "Structured lyrics"
                    </label>
                }
                if structured || multiline {
                    <textarea
                        id=(&input_id)
                        data-value=""
                        rows=(if structured { "3" } else { "2" })
                    >
                        (displayed)
                    </textarea>
                } else {
                    <input
                        id=(&input_id)
                        data-value=""
                        type=(if numeric { "number" } else { "text" })
                        step=(numeric.then_some("any"))
                        value=(displayed)
                        required=(required)
                    >
                }
                <div class="edit-diff" hidden=(true) aria-live="polite">
                    <del></del>
                    <span aria-hidden="true">"→"</span>
                    <ins></ins>
                </div>
                if has_history {
                    <div class="edit-history" id=(&history_id) hidden=(true)>
                        <h3>"Field history"</h3>
                        for revision in revisions {
                            <div class="revision">
                                <small>
                                    (minute_label(revision.minute))
                                    " · "
                                    (&revision.source)
                                </small>
                                <div class="edit-diff">
                                    <del>(&revision.old_value)</del>
                                    <span aria-hidden="true">"→"</span>
                                    <ins>(&revision.new_value)</ins>
                                </div>
                                <button
                                    type="button"
                                    data-restore-field=(&revision.old_value)
                                    data-minute=(revision.minute.to_string())
                                >
                                    "Use earlier value"
                                </button>
                            </div>
                        }
                    </div>
                }
            </div>
        }.boxed()
    }
}

fn history_button<'a>(cx: &'a Cx, id: &str, label: &str) -> BoxView<'a> {
    let id = id.to_owned();
    let label = label.to_owned();
    view! {
        <button
            class="history-icon"
            type="button"
            data-history=(&id)
            aria-controls=(&id)
            aria-expanded="false"
            aria-label=(&label)
            title=(&label)
        >
            <svg viewBox="0 0 24 24" aria-hidden="true">
                <path d="M3 12a9 9 0 1 1 2.5 6.2M3 6v6h6m3-5v5l3 2"></path>
            </svg>
        </button>
    }
    .boxed()
}

#[route(POST "/catalog/{collection}/{record_id}/edit")]
pub async fn edit_action(cx: &Cx, Form(form): Form<EditForm>) -> Result<Response> {
    if headers(cx)
        .get("sec-fetch-site")
        .is_some_and(|value| value != "same-origin" && value != "none")
    {
        return Ok(Response::builder()
            .status(StatusCode::FORBIDDEN)
            .body("Cross-origin edit rejected".into())?);
    }
    let Some(kind) = Kind::parse(collection(cx)).filter(|kind| Kind::MUSIC.contains(kind)) else {
        return Ok(Response::builder()
            .status(StatusCode::NOT_FOUND)
            .body("Record not found".into())?);
    };
    let id = record_id(cx);
    let db = store(cx);
    if form.action == "discography" && kind == Kind::Artist {
        db.queue_discography(id).await?;
        context::<Enrichment>(cx).wake_worker();
        return Ok(Response::builder()
            .status(StatusCode::SEE_OTHER)
            .header("location", format!("{}#artist-albums", kind.href(id)))
            .body(Body::default())?);
    }
    let result = apply(db, kind, id, form).await;
    match result {
        Ok(queued) => {
            if queued {
                context::<Enrichment>(cx).wake_worker();
            }
            Ok(Response::builder()
                .status(StatusCode::NO_CONTENT)
                .body(Body::default())?)
        }
        Err(error) => Ok(Response::builder()
            .status(StatusCode::BAD_REQUEST)
            .header("content-type", "text/plain; charset=utf-8")
            .body(error.to_string().into())?),
    }
}

async fn apply(db: &Store, kind: Kind, id: &str, form: EditForm) -> Result<bool> {
    if form.action != "apply" {
        return Err(topcoat::Error::msg("Unknown editor action"));
    }
    let batch: EditBatch = serde_json::from_str(form.value.as_deref().unwrap_or_default())?;
    db.apply_edits(kind, id, batch).await
}

fn revision_minutes(revisions: &[Revision]) -> Vec<i64> {
    let mut minutes: Vec<_> = revisions.iter().map(|revision| revision.minute).collect();
    minutes.dedup();
    minutes
}

fn minute_label(minute: i64) -> String {
    jiff::Timestamp::from_second(minute.saturating_mul(60))
        .map_or_else(|_| minute.to_string(), |date| date.to_string())
}
