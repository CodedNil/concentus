use super::super::artwork::web_url;
use super::Page;
use super::metadata::{artwork, fact, identifiers, related_link, role_label, schemes, tags};
use crate::catalog::Kind;
use concentus_types::{Artist, ArtistKind};
use topcoat::{
    context::Cx,
    view::{BoxView, ViewExt, view},
};

pub(super) fn render<'a>(cx: &'a Cx, page: &'a Page, artist: &'a Artist) -> BoxView<'a> {
    view! {
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
            (page.edit_fields(
                cx,
                &[
                    "sort_name",
                    "kind",
                    "begin_date",
                    "end_date",
                    "theme_background",
                    "theme_foreground",
                    "theme_accent",
                    "theme_font_family",
                    "theme_font_url",
                ],
            ))
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
        (page.edit_fields(cx, &["members"]))
        (tags(cx, &artist.tags))
        (page.edit_fields(cx, &["tags"]))
        (artwork(
            cx,
            [
                ("square", artist.artwork_square.as_deref()),
                ("banner", artist.artwork_banner.as_deref()),
                ("portrait", artist.artwork_portrait.as_deref()),
                ("motion banner", artist.artwork_motion_banner.as_deref()),
            ],
        ))
        (page.edit_fields(
            cx,
            &[
                "artwork_square",
                "artwork_banner",
                "artwork_portrait",
                "artwork_motion_banner",
            ],
        ))
        (identifiers(cx, Kind::Artist, &page.id, &artist.external_ids))
        (page.edit_fields(cx, &["external_ids"]))
    }
    .boxed()
}
