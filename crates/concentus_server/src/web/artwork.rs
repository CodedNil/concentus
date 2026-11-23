use crate::{catalog::Entry, enrichment::cover_art_archive};
use topcoat::{
    context::Cx,
    view::{BoxView, ViewExt, view},
};

pub fn candidates(entry: &Entry) -> Vec<String> {
    let mut candidates = Vec::new();
    if let Some(url) = entry.image.as_deref().and_then(web_url) {
        candidates.push(url.to_owned());
    }
    for (id, group) in [
        (entry.release_id.as_deref(), false),
        (entry.release_group_id.as_deref(), true),
    ] {
        if let Some(url) = id.and_then(|id| cover_art_archive::front(id, group))
            && !candidates.contains(&url)
        {
            candidates.push(url);
        }
    }
    candidates
}

pub fn image(cx: &Cx, urls: Vec<String>) -> BoxView<'_> {
    view! {
        if let Some(url) = urls.first() {
            <img
                src=(url)
                alt=""
                referrerpolicy="no-referrer"
                loading="lazy"
                data-next=(urls.get(1).map(String::as_str))
                data-last=(urls.get(2).map(String::as_str))
                onerror="if(this.dataset.next){this.src=this.dataset.next;this.dataset.next=this.dataset.last||'';delete this.dataset.last}else{this.remove()}"
            >
        }
    }.boxed()
}

pub fn web_url(raw: &str) -> Option<&str> {
    reqwest::Url::parse(raw)
        .ok()
        .and_then(|url| matches!(url.scheme(), "http" | "https").then_some(raw))
}
