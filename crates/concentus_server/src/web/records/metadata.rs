use super::{super::artwork::web_url, Page};
use crate::catalog::Kind;
use concentus_types::{
    Artwork, Credit, ExternalIdProvider, ExternalIds, RecordRef, Role, Scheme, Scored, Tag, TagKind,
};
use topcoat::{
    context::Cx,
    view::{BoxView, ViewExt, view},
};

pub(super) fn billed<'a>(cx: &'a Cx, page: &'a Page, credits: &'a [Credit]) -> BoxView<'a> {
    view! {
        <div class="billed">
            for credit in credits
                .iter()
                .filter(|credit| matches!(credit.role, Role::Primary | Role::Featured)) {
                (related_link(cx, page, Kind::Artist, &credit.artist_id.to_string()))
            }
        </div>
    }
    .boxed()
}

pub(super) fn credits<'a>(cx: &'a Cx, page: &'a Page, credits: &'a [Credit]) -> BoxView<'a> {
    view! {
        if !credits.is_empty() {
            <section class="panel">
                <div class="panel-heading"><h2>"Credits"</h2></div>
                <ul class="metadata-list">
                    for credit in credits {
                        <li>
                            (related_link(
                                cx,
                                page,
                                Kind::Artist,
                                &credit.artist_id.to_string(),
                            ))
                            <small>(role_label(&credit.role))</small>
                        </li>
                    }
                </ul>
            </section>
        }
    }
    .boxed()
}

pub(super) fn related_link<'a>(cx: &'a Cx, page: &Page, kind: Kind, id: &str) -> BoxView<'a> {
    let href = kind.href(id);
    let entry = page.names.get(&href);
    let title = entry.map_or_else(|| kind.label().to_owned(), |entry| entry.title.clone());
    let text = entry.map_or_else(
        || kind.label().to_owned(),
        |entry| {
            [entry.subtitle.as_deref(), entry.preview.as_deref()]
                .into_iter()
                .flatten()
                .collect::<Vec<_>>()
                .join(" · ")
        },
    );
    tooltip_link(cx, href, title, text)
}

fn tooltip_link(cx: &Cx, href: String, title: String, text: String) -> BoxView<'_> {
    view! {
        <span class="has-tip">
            <a href=(&href) title=(&text)>(&title)</a>
            <span class="tip" aria-hidden="true">
                <strong>(&title)</strong>
                <small>(&text)</small>
            </span>
        </span>
    }
    .boxed()
}

pub(super) fn tags<'a>(cx: &'a Cx, tags: &'a [Scored<RecordRef<Tag>>]) -> BoxView<'a> {
    let visible = tags.iter().filter_map(|weighted| match &weighted.id {
        RecordRef::Data(tag) if tag.kind != TagKind::Hidden => Some((tag, weighted.weight)),
        _ => None,
    });
    view! {
        if visible.clone().next().is_some() {
            <section class="panel">
                <div class="panel-heading"><h2>"Tags"</h2></div>
                <div class="chips">
                    for (tag, weight) in visible {
                        <button
                            type="button"
                            class="tag-chip tag-trigger has-tip"
                            data-tag-detail=(tag.id.to_string())
                            aria-haspopup="dialog"
                            style=(format!("--tag-weight: {weight}%"))
                        >
                            <span class="tag-weight-orbit" aria-hidden="true"></span>
                            (&tag.name)
                            <span class="tip">
                                <strong>(&tag.name)</strong>
                                <small>
                                    (format!("{:?} · Weight {} / 100", tag.kind, weight))
                                </small>
                            </span>
                        </button>
                    }
                </div>
            </section>
        }
    }
    .boxed()
}

pub(super) fn artwork<'a, const N: usize>(
    cx: &'a Cx,
    artwork: [(&'a str, Option<&'a Artwork>); N],
) -> BoxView<'a> {
    view! {
        if artwork.iter().any(|(_, art)| art.is_some()) {
            <section class="panel">
                <div class="panel-heading"><h2>"Artwork"</h2></div>
                <div class="gallery">
                    for (label, art) in artwork
                        .iter()
                        .filter_map(|(label, art)| art.map(|art| (*label, art))) {
                        <article>
                            <h3>
                                (label)
                                if art.explicit {
                                    <small>" · Explicit"</small>
                                }
                            </h3>
                            for file in &art.files {
                                if let Some(url) = web_url(&file.url) {
                                    <a href=(url) rel="noreferrer">
                                        (format!("{} × {}", file.width, file.height))
                                        if let Some(codec) = &file.codec {
                                            " · "
                                            (codec)
                                        }
                                        if let Some(ms) = file.duration_ms {
                                            " · "
                                            (duration(ms))
                                        }
                                    </a>
                                }
                            }
                            if !art.colors.is_empty() {
                                <small>
                                    (art
                                        .colors
                                        .iter()
                                        .map(ToString::to_string)
                                        .collect::<Vec<_>>()
                                        .join(" · "))
                                </small>
                            }
                            if let Some(hash) = &art.blurhash {
                                <small>
                                    "Blurhash "
                                    (hash)
                                </small>
                            }
                        </article>
                    }
                </div>
            </section>
        }
    }
    .boxed()
}

pub(super) fn schemes<'a>(cx: &'a Cx, scheme: Option<&'a Scheme>) -> BoxView<'a> {
    view! {
        if let Some(scheme) = scheme {
            <ul class="palette" aria-label="Theme colors">
                for (label, color) in [
                    ("Background", scheme.background),
                    ("Foreground", scheme.foreground),
                    ("Accent", scheme.accent),
                ] {
                    <li>
                        <span style=(format!("--swatch: {color}")) aria-hidden="true"></span>
                        (label)
                        " "
                        (color.to_string())
                    </li>
                }
            </ul>
        }
    }
    .boxed()
}

pub(super) fn identifiers<'a>(
    cx: &'a Cx,
    kind: Kind,
    id: &'a str,
    ids: &'a ExternalIds,
) -> BoxView<'a> {
    let mut ids = ids
        .iter()
        .map(|(provider, values)| (provider_label(provider), provider, values))
        .collect::<Vec<_>>();
    ids.sort_unstable_by(|a, b| a.0.cmp(&b.0));
    view! {
        <section class="panel">
            <div class="panel-heading"><h2>"Identifiers"</h2></div>
            <dl class="facts identifiers">
                (fact(cx, "Concentus", Some(id)))
                for (label, provider, values) in &ids {
                    <div>
                        <dt>(label)</dt>
                        <dd>
                            for value in *values {
                                if let Some(link) = source_link(kind, provider, value) {
                                    <a href=(link) rel="noreferrer">
                                        (value)
                                        " ↗"
                                    </a>
                                } else {
                                    <span>(value)</span>
                                }
                            }
                        </dd>
                    </div>
                }
            </dl>
        </section>
    }
    .boxed()
}

pub(super) fn fact<'a>(cx: &'a Cx, label: &str, value: Option<&str>) -> BoxView<'a> {
    let label = label.to_owned();
    let value = value.map(str::to_owned);
    view! {
        if let Some(value) = &value {
            <div>
                <dt>(&label)</dt>
                <dd>(value)</dd>
            </div>
        }
    }
    .boxed()
}

fn source_link(kind: Kind, provider: &ExternalIdProvider, value: &str) -> Option<String> {
    if !value
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
    {
        return None;
    }
    let (domain, path) = match (provider, kind) {
        (ExternalIdProvider::Musicbrainz, Kind::Artist) => ("musicbrainz.org", "artist"),
        (ExternalIdProvider::Musicbrainz, Kind::Album) => ("musicbrainz.org", "release"),
        (ExternalIdProvider::Musicbrainz, Kind::Track) => ("musicbrainz.org", "recording"),
        (ExternalIdProvider::Spotify, Kind::Artist) => ("open.spotify.com", "artist"),
        (ExternalIdProvider::Spotify, Kind::Album) => ("open.spotify.com", "album"),
        (ExternalIdProvider::Spotify, Kind::Track) => ("open.spotify.com", "track"),
        (ExternalIdProvider::Wikidata, _) => ("www.wikidata.org", "wiki"),
        _ => return None,
    };
    Some(format!("https://{domain}/{path}/{value}"))
}

fn provider_label(provider: &ExternalIdProvider) -> String {
    match provider {
        ExternalIdProvider::Musicbrainz => "MusicBrainz".into(),
        ExternalIdProvider::Isrc => "ISRC".into(),
        ExternalIdProvider::Isni => "ISNI".into(),
        ExternalIdProvider::Ipi => "IPI".into(),
        ExternalIdProvider::AppleMusic => "Apple Music".into(),
        ExternalIdProvider::YoutubeMusic => "YouTube Music".into(),
        ExternalIdProvider::Other(name) => name.clone(),
        _ => format!("{provider:?}"),
    }
}

pub(super) fn role_label(role: &Role) -> String {
    match role {
        Role::Instrument(label) | Role::Other(label) => label.clone(),
        Role::BackgroundVocals => "Background vocals".into(),
        Role::ExecutiveProducer => "Executive producer".into(),
        _ => format!("{role:?}"),
    }
}

pub(super) fn duration(ms: u32) -> String {
    let seconds = ms / 1000;
    format!("{}:{:02}", seconds / 60, seconds % 60)
}
