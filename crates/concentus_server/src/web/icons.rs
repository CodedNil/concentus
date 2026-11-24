use crate::catalog::Kind;
use topcoat::{
    context::Cx,
    view::{BoxView, ViewExt, view},
};

pub(super) fn icon(cx: &Cx, kind: Option<Kind>) -> BoxView<'_> {
    view! {
        <svg class="catalog-icon" viewBox="0 0 48 48" fill="none" aria-hidden="true">
            match kind {
                Some(Kind::Artist) => {
                    <circle
                        class="icon-orbit"
                        cx="24"
                        cy="24"
                        r="20"
                        stroke-dasharray="21 9"
                    ></circle>
                    <circle cx="24" cy="19" r="6"></circle>
                    <path d="M13 35c1-6 5-9 11-9s10 3 11 9"></path>
                }
                Some(Kind::Album) => {
                    <circle cx="24" cy="24" r="19"></circle>
                    <g class="icon-orbit">
                        <circle cx="24" cy="24" r="13" stroke-dasharray="21 6"></circle>
                        <circle cx="24" cy="24" r="7"></circle>
                    </g>
                    <circle cx="24" cy="24" r="2"></circle>
                }
                Some(Kind::Track) => {
                    <path class="wave wave-low" d="M6 20v8m36-8v8"></path>
                    <path class="wave wave-mid" d="M15 14v20m18-20v20"></path>
                    <path class="wave wave-high" d="M24 6v36"></path>
                }
                Some(Kind::Tag) => {
                    <g class="icon-tag">
                        <path d="M9 26 26 9h12v12L21 38z"></path>
                        <circle cx="31" cy="16" r="2"></circle>
                    </g>
                }
                None => {
                    <path d="M24 4 44 24 24 44 4 24z"></path>
                    <circle
                        class="icon-orbit"
                        cx="24"
                        cy="24"
                        r="10"
                        stroke-dasharray="18 8"
                    ></circle>
                    <circle cx="24" cy="24" r="3"></circle>
                }
            }
        </svg>
    }
    .boxed()
}
