use super::Page;
use super::metadata::{artwork, credits, fact, identifiers, related_link, schemes, tags};
use crate::catalog::Kind;
use concentus_types::{Album, AlbumType, RecordRef};
use topcoat::{
    context::Cx,
    view::{BoxView, ViewExt, view},
};

pub(super) fn render<'a>(cx: &'a Cx, page: &'a Page, album: &'a Album) -> BoxView<'a> {
    view! {
        <section class="panel">
            <div class="panel-heading"><h2>"Release details"</h2></div>
            <dl class="facts">
                (fact(
                    cx,
                    "Release date",
                    album.release_date.map(|date| date.to_string()).as_deref(),
                ))
                (fact(cx, "Sort title", album.sort_title.as_deref()))
                (fact(cx, "Type", album.album_type.as_ref().map(album_type).as_deref()))
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
            (page.edit_fields(
                cx,
                &[
                    "release_date",
                    "sort_title",
                    "album_type",
                    "label",
                    "copyright",
                    "edition_name",
                    "edition_of",
                    "soundtrack_source",
                    "theme_background",
                    "theme_foreground",
                    "theme_accent",
                ],
            ))
            (schemes(cx, album.theme.as_ref()))
        </section>
        if !album.discs.is_empty() {
            <section class="panel">
                <div class="panel-heading">
                    <h2>"Tracklist"</h2>
                    <small>
                        (album.track_count())
                        (if album.track_count() == 1 { " track" } else { " tracks" })
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
        (page.edit_fields(cx, &["discs"]))
        (credits(cx, page, &album.credits))
        (page.edit_fields(cx, &["credits"]))
        (tags(cx, &album.tags))
        (page.edit_fields(cx, &["tags"]))
        (artwork(
            cx,
            [
                ("square", album.artwork_square.as_deref()),
                ("banner", album.artwork_banner.as_deref()),
                ("motion square", album.artwork_motion_square.as_deref()),
            ],
        ))
        (page.edit_fields(
            cx,
            &["artwork_square", "artwork_banner", "artwork_motion_square"],
        ))
        (identifiers(cx, Kind::Album, &page.id, &album.external_ids))
        (page.edit_fields(cx, &["external_ids"]))
    }
    .boxed()
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
