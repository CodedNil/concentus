use super::{
    artwork::{self, web_url},
    entry_list, icon,
};
use crate::{
    catalog::{Detail, Entry, Kind},
    store::Store,
};
use concentus_types::{
    AlbumType, ArtistKind, Artwork, Credit, DerivationKind, ExternalIdProvider, ExternalIds,
    RecordRef, Role, Scheme, Scored, Tag,
};
use std::collections::HashMap;
use topcoat::{
    Result,
    context::Cx,
    view::{BoxView, ViewExt, view},
};

pub struct Page {
    pub record: Option<Detail>,
    id: String,
    title: String,
    kind: Kind,
    names: HashMap<String, Entry>,
    related: Vec<(Kind, Vec<Entry>)>,
    images: Vec<String>,
}

impl Page {
    pub async fn load(db: &Store, record: Option<Detail>) -> Result<(String, Self)> {
        let mut references = Vec::new();
        let mut artist_credits = |credits: &[Credit]| {
            references.extend(
                credits
                    .iter()
                    .map(|credit| (Kind::Artist, credit.artist_id.to_string())),
            );
        };
        let (kind, id, title) = match &record {
            Some(Detail::Artist(artist)) => {
                if let ArtistKind::Group { members } = &artist.kind {
                    references.extend(
                        members
                            .iter()
                            .map(|member| (Kind::Artist, member.artist_id.to_string())),
                    );
                }
                (Kind::Artist, artist.id.to_string(), artist.name.clone())
            }
            Some(Detail::Album(album)) => {
                artist_credits(&album.credits);
                references.extend(album.track_ids().map(|id| (Kind::Track, id.to_string())));
                if let Some(base) = album.edition.as_ref().and_then(|edition| edition.of) {
                    references.push((Kind::Album, base.to_string()));
                }
                (Kind::Album, album.id.to_string(), album.display_title())
            }
            Some(Detail::Track(track)) => {
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
            Some(Detail::Tag(tag)) => {
                if let Some(parent) = tag.parent {
                    references.push((Kind::Tag, parent.to_string()));
                }
                (Kind::Tag, tag.id.to_string(), tag.name.clone())
            }
            None => (Kind::Track, String::new(), "Record not found".to_owned()),
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
            Some(Detail::Artist(_)) => &[Kind::Album, Kind::Track, Kind::Artist][..],
            Some(Detail::Track(_)) => &[Kind::Album][..],
            Some(Detail::Tag(_)) => Kind::ALL.as_slice(),
            _ => &[],
        };
        for &target in targets {
            let entries = db.entries(target, "", "", Some((kind, &id)), 50).await?;
            if !entries.is_empty() {
                related.push((target, entries));
            }
        }
        let images = if record.is_some() {
            db.entries_by_id(kind, std::slice::from_ref(&id))
                .await?
                .first()
                .map_or_else(Vec::new, artwork::candidates)
        } else {
            Vec::new()
        };
        Ok((
            title.clone(),
            Self {
                record,
                id,
                title,
                kind,
                names,
                related,
                images,
            },
        ))
    }
}

pub fn render<'a>(cx: &'a Cx, page: &'a Page) -> BoxView<'a> {
    view! {
        if let Some(record) = &page.record {
            <nav class="crumbs" aria-label="Breadcrumb">
                <a href="/">"Catalog"</a>
                <span aria-hidden="true">"/"</span>
                <a href=(format!("/?kind={}", page.kind.path()))>
                    (page.kind.plural())
                </a>
            </nav>
            <div class="detail">
                (heading(cx, page, record))
                (metadata(cx, page, record))
                for (kind, entries) in &page.related {
                    <section class="panel">
                        <div class="panel-heading">
                            <h2>
                                (match (page.kind, *kind) {
                                    (Kind::Track, Kind::Album) => "Appears on",
                                    (Kind::Artist, Kind::Artist) => "Groups",
                                    (Kind::Tag, Kind::Tag) => "Narrower tags",
                                    _ => kind.plural(),
                                })
                            </h2>
                        </div>
                        (entry_list(cx, *kind, entries))
                    </section>
                }
            </div>
        } else {
            <section class="empty">
                <h1>"Record not found"</h1>
                <p>"This record is unavailable in the local catalog."</p>
                <a href="/">"Return to the catalog"</a>
            </section>
        }
    }
    .boxed()
}

fn heading<'a>(cx: &'a Cx, page: &'a Page, record: &'a Detail) -> BoxView<'a> {
    view! {
        if Kind::MUSIC.contains(&page.kind) {
            <a class="edit-link" href=(format!("{}/edit", page.kind.href(&page.id)))>
                "Edit metadata"
            </a>
        }
        <section class="record-heading">
            <div class="cover" aria-hidden="true">
                (icon(page.kind))
                (artwork::image(cx, page.images.clone()))
            </div>
            <div>
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
                    Detail::Tag(tag) => {
                        <p>(format!("{:?}", tag.kind))</p>
                    }
                }
            </div>
        </section>
    }
    .boxed()
}

fn metadata<'a>(cx: &'a Cx, page: &'a Page, record: &'a Detail) -> BoxView<'a> {
    view! {
        match record {
            Detail::Artist(artist) => {
                <section class="panel">
                    <div class="panel-heading"><h2>"Artist details"</h2></div>
                    <dl class="facts">
                        (fact(cx, "Sort name", artist.sort_name.as_deref()))
                        (fact(
                            cx,
                            "Type",
                            Some(
                                match artist.kind {
                                    ArtistKind::Person => "Person",
                                    ArtistKind::Group { .. } => "Group",
                                    ArtistKind::Unknown => "Unknown",
                                },
                            ),
                        ))
                        (fact(
                            cx,
                            "Born or formed",
                            artist.begin.map(|date| date.to_string()).as_deref(),
                        ))
                        (fact(
                            cx,
                            "Died or disbanded",
                            artist.end.map(|date| date.to_string()).as_deref(),
                        ))
                        if let Some(font) = &artist.font {
                            <div>
                                <dt>"Font"</dt>
                                <dd>
                                    if let Some(url) = web_url(&font.url) {
                                        <a href=(url)>(&font.family)</a>
                                    } else {
                                        (&font.family)
                                    }
                                </dd>
                            </div>
                        }
                    </dl>
                    (schemes(cx, artist.theme.as_ref()))
                </section>
                if let ArtistKind::Group { members } = &artist.kind {
                    <section class="panel">
                        <div class="panel-heading"><h2>"Members"</h2></div>
                        <ul class="metadata-list">
                            for member in members {
                                <li>
                                    (related_link(
                                        cx,
                                        page,
                                        Kind::Artist,
                                        &member.artist_id.to_string(),
                                    ))
                                    <small>
                                        (member
                                            .roles
                                            .iter()
                                            .map(role_label)
                                            .collect::<Vec<_>>()
                                            .join(", "))
                                    </small>
                                    if member.joined.is_some() || member.left.is_some() {
                                        <small>
                                            (member.joined.map_or_else(
                                                || "Unknown".into(),
                                                |date| date.to_string(),
                                            ))
                                            " – "
                                            (member.left.map_or_else(
                                                || "Unknown".into(),
                                                |date| date.to_string(),
                                            ))
                                        </small>
                                    }
                                </li>
                            }
                        </ul>
                    </section>
                }
                (tags(cx, &artist.tags))
                (artwork(cx, &artist.artwork))
                (identifiers(cx, Kind::Artist, &page.id, &artist.external_ids))
            }
            Detail::Album(album) => {
                <section class="panel">
                    <div class="panel-heading"><h2>"Release details"</h2></div>
                    <dl class="facts">
                        (fact(
                            cx,
                            "Release date",
                            album.release_date.map(|date| date.to_string()).as_deref(),
                        ))
                        (fact(cx, "Sort title", album.sort_title.as_deref()))
                        (fact(
                            cx,
                            "Type",
                            album.album_type.as_ref().map(album_type).as_deref(),
                        ))
                        (fact(cx, "Label", album.label.as_deref()))
                        (fact(cx, "Copyright", album.copyright.as_deref()))
                        if let Some(edition) = &album.edition {
                            (fact(cx, "Edition", Some(&edition.name)))
                            if let Some(base) = edition.of {
                                <div>
                                    <dt>"Base release"</dt>
                                    <dd>
                                        (related_link(cx, page, Kind::Album, &base.to_string()))
                                    </dd>
                                </div>
                            }
                        }
                    </dl>
                    (schemes(cx, album.theme.as_ref()))
                </section>
                if !album.discs.is_empty() {
                    <section class="panel">
                        <div class="panel-heading">
                            <h2>"Tracklist"</h2>
                            <small>
                                (album.track_count())
                                (if album.track_count() == 1 {
                                    " track"
                                } else {
                                    " tracks"
                                })
                            </small>
                        </div>
                        for (index, disc) in album.discs.iter().enumerate() {
                            <h3 class="subheading">
                                "Disc "
                                (index + 1)
                                if let Some(title) = &disc.title {
                                    " · "
                                    (title)
                                }
                            </h3>
                            <ol class="metadata-list ordered">
                                for entry in &disc.tracks {
                                    if let RecordRef::Id(id) = entry {
                                        <li>
                                            (related_link(cx, page, Kind::Track, &id.to_string()))
                                        </li>
                                    }
                                }
                            </ol>
                        }
                    </section>
                }
                (credits(cx, page, &album.credits))
                (tags(cx, &album.tags))
                (artwork(cx, &album.artwork))
                (identifiers(cx, Kind::Album, &page.id, &album.external_ids))
            }
            Detail::Track(track) => {
                <section class="panel">
                    <div class="panel-heading"><h2>"Recording details"</h2></div>
                    <dl class="facts">
                        if track.duration_ms > 0 {
                            (fact(cx, "Duration", Some(&duration(track.duration_ms))))
                        }
                        (fact(cx, "Version", track.version.as_deref()))
                        (fact(
                            cx,
                            "Release date",
                            track.release_date.map(|date| date.to_string()).as_deref(),
                        ))
                        (fact(
                            cx,
                            "Content rating",
                            track.rating.map(|rating| format!("{rating:?}")).as_deref(),
                        ))
                        if !track.languages.is_empty() {
                            (fact(cx, "Languages", Some(&track.languages.join(", "))))
                        }
                        if let Some(url) = track
                            .music_video
                            .as_deref()
                            .and_then(web_url) {
                            <div>
                                <dt>"Music video"</dt>
                                <dd><a href=(url)>"Watch video ↗"</a></dd>
                            </div>
                        }
                    </dl>
                    if let Some(about) = &track.about {
                        <p class="panel-copy prose">(about)</p>
                    }
                </section>
                (credits(cx, page, &track.credits))
                if !track.lyrics.is_empty() {
                    <section class="panel">
                        <div class="panel-heading"><h2>"Lyrics"</h2></div>
                        for (language, lyrics) in sorted(&track.lyrics) {
                            <details open=(true) class="lyric">
                                <summary>(language)</summary>
                                if lyrics.instrumental {
                                    <p>"Instrumental"</p>
                                }
                                for channel in &lyrics.channels {
                                    <p class="channel">
                                        (format!("{:?}", channel.kind))
                                        if let Some(id) = channel.artist_id {
                                            " · "
                                            (related_link(cx, page, Kind::Artist, &id.to_string()))
                                        }
                                    </p>
                                    <pre>
                                        (channel
                                            .spans
                                            .iter()
                                            .map(|span| span.text.as_str())
                                            .collect::<String>())
                                    </pre>
                                }
                            </details>
                        }
                    </section>
                }
                (audio(cx, &track.features))
                if !track.derived_from.is_empty() {
                    <section class="panel">
                        <div class="panel-heading"><h2>"Derived from"</h2></div>
                        <ul class="metadata-list">
                            for (id, kinds) in sorted(&track.derived_from) {
                                <li>
                                    (related_link(cx, page, Kind::Track, &id.to_string()))
                                    <small>
                                        (kinds
                                            .iter()
                                            .map(derivation)
                                            .collect::<Vec<_>>()
                                            .join(" · "))
                                    </small>
                                </li>
                            }
                        </ul>
                    </section>
                }
                (tags(cx, &track.tags))
                (artwork(cx, &track.artwork))
                (identifiers(cx, Kind::Track, &page.id, &track.external_ids))
            }
            Detail::Tag(tag) => {
                <section class="panel">
                    <div class="panel-heading"><h2>"Tag details"</h2></div>
                    <dl class="facts">
                        (fact(cx, "Type", Some(&format!("{:?}", tag.kind))))
                        if !tag.aliases.is_empty() {
                            (fact(cx, "Aliases", Some(&tag.aliases.join(", "))))
                        }
                        if let Some(parent) = tag.parent {
                            <div>
                                <dt>"Parent tag"</dt>
                                <dd>
                                    (related_link(cx, page, Kind::Tag, &parent.to_string()))
                                </dd>
                            </div>
                        }
                        (fact(cx, "Concentus ID", Some(&page.id)))
                    </dl>
                </section>
            }
        }
    }
    .boxed()
}

fn billed<'a>(cx: &'a Cx, page: &'a Page, credits: &'a [Credit]) -> BoxView<'a> {
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

fn credits<'a>(cx: &'a Cx, page: &'a Page, credits: &'a [Credit]) -> BoxView<'a> {
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

fn related_link<'a>(cx: &'a Cx, page: &Page, kind: Kind, id: &str) -> BoxView<'a> {
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

fn tags<'a>(cx: &'a Cx, tags: &'a [Scored<RecordRef<Tag>>]) -> BoxView<'a> {
    view! {
        if !tags.is_empty() {
            <section class="panel">
                <div class="panel-heading"><h2>"Tags"</h2></div>
                <div class="chips">
                    for weighted in tags {
                        if let RecordRef::Data(tag) = &weighted.id {
                            (tooltip_link(
                                cx,
                                Kind::Tag.href(tag.id),
                                tag.name.clone(),
                                format!(
                                    "{:?} · Weight {} / 100",
                                    tag.kind,
                                    weighted.weight,
                                ),
                            ))
                        }
                    }
                </div>
            </section>
        }
    }
    .boxed()
}

fn artwork<'a>(cx: &'a Cx, artwork: &'a [Artwork]) -> BoxView<'a> {
    view! {
        if !artwork.is_empty() {
            <section class="panel">
                <div class="panel-heading"><h2>"Artwork"</h2></div>
                <div class="gallery">
                    for art in artwork {
                        <article>
                            <h3>
                                (format!("{:?}", art.kind))
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

fn schemes<'a>(cx: &'a Cx, scheme: Option<&'a Scheme>) -> BoxView<'a> {
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

fn identifiers<'a>(cx: &'a Cx, kind: Kind, id: &'a str, ids: &'a ExternalIds) -> BoxView<'a> {
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

fn fact<'a>(cx: &'a Cx, label: &str, value: Option<&str>) -> BoxView<'a> {
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

fn audio<'a>(cx: &'a Cx, features: &'a concentus_types::AudioFeatures) -> BoxView<'a> {
    let values = [
        (
            "Tempo",
            features.tempo_bpm.map(|value| format!("{value} BPM")),
        ),
        ("Key", features.key.map(|value| format!("{value:?}"))),
        ("Mode", features.mode.map(|value| format!("{value:?}"))),
        (
            "Beats per bar",
            features.time_signature.map(|value| value.to_string()),
        ),
        (
            "Loudness",
            features.loudness_lufs.map(|value| format!("{value} LUFS")),
        ),
        ("Peak", features.peak.map(|value| value.to_string())),
        (
            "Acousticness",
            features.acousticness.map(|value| format!("{value}%")),
        ),
        (
            "Danceability",
            features.danceability.map(|value| format!("{value}%")),
        ),
        ("Energy", features.energy.map(|value| format!("{value}%"))),
        (
            "Instrumentalness",
            features.instrumentalness.map(|value| format!("{value}%")),
        ),
        (
            "Liveness",
            features.liveness.map(|value| format!("{value}%")),
        ),
        (
            "Speechiness",
            features.speechiness.map(|value| format!("{value}%")),
        ),
        ("Valence", features.valence.map(|value| format!("{value}%"))),
    ];
    view! {
        if values.iter().any(|(_, value)| value.is_some()) {
            <section class="panel">
                <div class="panel-heading"><h2>"Audio analysis"</h2></div>
                <dl class="facts">
                    for (label, value) in &values {
                        (fact(cx, label, value.as_deref()))
                    }
                </dl>
            </section>
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

fn role_label(role: &Role) -> String {
    match role {
        Role::Instrument(label) | Role::Other(label) => label.clone(),
        Role::BackgroundVocals => "Background vocals".into(),
        Role::ExecutiveProducer => "Executive producer".into(),
        _ => format!("{role:?}"),
    }
}

fn album_type(kind: &AlbumType) -> String {
    match kind {
        AlbumType::Ep => "EP".into(),
        AlbumType::Soundtrack {
            source: Some(source),
        } => format!("Soundtrack · {source}"),
        AlbumType::Soundtrack { source: None } => "Soundtrack".into(),
        _ => format!("{kind:?}"),
    }
}

fn derivation(kind: &DerivationKind) -> String {
    match kind {
        DerivationKind::Sample {
            from_range,
            to_range,
        } => {
            let span = |range: &concentus_types::TimeRange| {
                format!("{}–{} ms", range.start_ms, range.end_ms)
            };
            let mut parts = vec!["Sample".to_owned()];
            if let Some(range) = from_range {
                parts.push(format!("here {}", span(range)));
            }
            if let Some(range) = to_range {
                parts.push(format!("source {}", span(range)));
            }
            parts.join(" · ")
        }
        _ => format!("{kind:?}"),
    }
}

fn duration(ms: u32) -> String {
    let seconds = ms / 1000;
    format!("{}:{:02}", seconds / 60, seconds % 60)
}

fn sorted<K: Ord, V>(entries: &HashMap<K, V>) -> Vec<(&K, &V)> {
    let mut entries: Vec<_> = entries.iter().collect();
    entries.sort_unstable_by_key(|(key, _)| *key);
    entries
}
