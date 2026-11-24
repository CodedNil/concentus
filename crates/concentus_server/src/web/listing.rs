use super::{artwork, icons::icon};
use crate::catalog::{Entry, Kind};
use topcoat::{
    context::Cx,
    view::{BoxView, ViewExt, view},
};

pub(super) fn entry_list<'a>(cx: &'a Cx, kind: Kind, entries: &'a [Entry]) -> BoxView<'a> {
    view! {
        <ul class="entries" data-kind=(kind.path())>
            for entry in entries {
                <li>
                    if kind == Kind::Tag {
                        <button
                            type="button"
                            class="entry has-tip tag-trigger"
                            data-tag-detail=(&entry.id)
                            aria-haspopup="dialog"
                        >
                            <span class="entry-icon">(icon(cx, Some(Kind::Tag)))</span>
                            <span class="entry-copy">
                                <strong>(&entry.title)</strong>
                            </span>
                            if let Some(preview) = &entry.preview {
                                <span class="tip">
                                    <strong>(&entry.title)</strong>
                                    <small>(preview)</small>
                                </span>
                            }
                        </button>
                    } else {
                        <a
                            class="entry has-tip"
                            href=(kind.href(&entry.id))
                            title=(entry.preview.as_deref())
                        >
                            <span class="entry-icon" aria-hidden="true">
                                <span class="art-face">
                                    (icon(cx, Some(kind)))
                                    (artwork::image(cx, artwork::candidates(entry)))
                                </span>
                            </span>
                            <span class="entry-copy">
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
                    }
                </li>
            }
        </ul>
    }
    .boxed()
}
