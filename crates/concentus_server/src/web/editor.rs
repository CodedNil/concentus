use super::{collection, layout, record_id};
use crate::{catalog::Kind, store::store};
use serde::Deserialize;
use topcoat::{
    Result,
    context::{Cx, context},
    router::{Body, StatusCode, content::Form, page, request::headers, response::Response, route},
    view::{View, ViewExt, view},
};

#[derive(Deserialize)]
struct EditForm {
    action: String,
    field: Option<String>,
    value: Option<String>,
    language: Option<String>,
}

#[page("/catalog/{collection}/{record_id}/edit")]
pub async fn edit_page(cx: &Cx) -> Result<impl View> {
    let kind = Kind::parse(collection(cx)).filter(|kind| Kind::MUSIC.contains(kind));
    let id = record_id(cx);
    let data = if let Some(kind) = kind {
        Some(store(cx).editor(kind, id).await?)
    } else {
        None
    };
    let record = kind.map_or_else(String::new, |kind| kind.href(id));
    let action = format!("{record}/edit");
    let title = data
        .as_ref()
        .and_then(|(fields, _, _)| fields.get("title").or_else(|| fields.get("name")))
        .and_then(serde_json::Value::as_str)
        .map_or_else(|| "Edit metadata".into(), |name| format!("Edit {name}"));
    Ok(layout(cx, title.clone(), kind, String::new(), view! {
        if let Some((fields, revisions, pending)) = &data && !fields.is_empty() {
            <nav class="crumbs"><a href=(&record)>"Back to record"</a></nav>
            <div class="page-heading"><h1>(&title)</h1></div>
            <p class="notice">
                "Use JSON values for fields. Related rows are grouped by table. Use null to clear an optional field."
            </p>
            <section class="panel editor-panel">
                <form method="post" action=(&action)>
                    <button name="action" value="queue">"Refresh all fields"</button>
                </form>
                <details>
                    <summary>"Edit related fields together"</summary>
                    <form method="post" action=(&action)>
                        <textarea name="value" rows="12" aria-label="Record fields">
                            (serde_json::to_string_pretty(fields).unwrap_or_default())
                        </textarea>
                        <button name="action" value="save_all">"Save fields"</button>
                    </form>
                </details>
                <details>
                    <summary>"Record history"</summary>
                    for minute in revision_minutes(revisions) {
                        <form method="post" action=(&action)>
                            <input
                                type="hidden"
                                name="value"
                                value=((minute - 1).to_string())
                            >
                            <button name="action" value="restore">
                                "Restore before "
                                (minute_label(minute))
                            </button>
                        </form>
                    }
                    if revisions.is_empty() {
                        <p>"No changes recorded yet."</p>
                    }
                </details>
            </section>
            for (field, value) in fields {
                <section class="panel editor-panel">
                    <h2>(field.replace('_', " "))</h2>
                    if pending.contains(field) {
                        <small>"Enrichment queued"</small>
                    }
                    if field == "lyrics" {
                        <form method="post" action=(&action)>
                            <label>
                                "Language "
                                <input name="language" placeholder="en" required=(true)>
                            </label>
                            <textarea name="value" aria-label="Lyrics text" rows="5"></textarea>
                            <button name="action" value="lyrics">
                                "Save lyrics in this language"
                            </button>
                            <small>
                                "An empty value removes lyrics for this language."
                            </small>
                        </form>
                    }
                    <form method="post" action=(&action)>
                        <input type="hidden" name="field" value=(field)>
                        <textarea
                            name="value"
                            aria-label=(field)
                            rows=(if value.is_object() { "8" } else { "2" })
                        >
                            (serde_json::to_string_pretty(value).unwrap_or_default())
                        </textarea>
                        <button name="action" value="save">"Save"</button>
                        <button name="action" value="queue">"Refresh field"</button>
                    </form>
                    <details>
                        <summary>"Field history"</summary>
                        for revision in revisions
                            .iter()
                            .filter(|revision| &revision.field == field) {
                            <div class="revision">
                                <p>(minute_label(revision.minute))</p>
                                <pre>(&revision.old_value)</pre>
                                <pre>(&revision.new_value)</pre>
                                <form method="post" action=(&action)>
                                    <input type="hidden" name="field" value=(field)>
                                    <input
                                        type="hidden"
                                        name="value"
                                        value=((revision.minute - 1).to_string())
                                    >
                                    <button name="action" value="restore">
                                        "Restore earlier value"
                                    </button>
                                </form>
                            </div>
                        }
                    </details>
                </section>
            }
        } else {
            (StatusCode::NOT_FOUND)
            <div class="empty"><h1>"Record not found"</h1></div>
        }
    }.boxed()))
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
    let value = form.value.as_deref().unwrap_or_default();
    let result = if kind == Kind::Track && form.action == "lyrics" {
        db.save_lyrics(id, form.language.as_deref().unwrap_or_default(), value)
            .await
    } else {
        db.edit(
            kind,
            id,
            &form.action,
            form.field.as_deref().unwrap_or_default(),
            value,
        )
        .await
    };
    match result {
        Ok(()) => {
            if form.action == "queue" {
                context::<crate::enrichment::Enrichment>(cx).wake_worker();
            }
            Ok(Response::builder()
                .status(StatusCode::SEE_OTHER)
                .header("location", format!("{}/edit", kind.href(id)))
                .body(Body::default())?)
        }
        Err(error) => Ok(Response::builder()
            .status(StatusCode::BAD_REQUEST)
            .header("content-type", "text/plain; charset=utf-8")
            .body(format!("Could not apply edit: {error}").into())?),
    }
}

fn revision_minutes(revisions: &[crate::editor::Revision]) -> Vec<i64> {
    let mut result = revisions
        .iter()
        .map(|revision| revision.minute)
        .collect::<Vec<_>>();
    result.dedup();
    result
}

fn minute_label(minute: i64) -> String {
    // SQLite records UTC Unix minute buckets.
    let seconds = minute.saturating_mul(60);
    jiff::Timestamp::from_second(seconds)
        .map_or_else(|_| minute.to_string(), |date| date.to_string())
}
