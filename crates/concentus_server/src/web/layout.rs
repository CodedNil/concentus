use super::icons::icon;
use crate::catalog::Kind;
use topcoat::{
    context::Cx,
    view::{BoxView, View, ViewExt, view},
};

pub(super) fn catalog_heading<'a>(
    cx: &'a Cx,
    title: &'a str,
    collection: Option<Kind>,
    overview: bool,
) -> BoxView<'a> {
    view! {
        <section
            class="page-heading"
            data-kind=(collection.map_or("overview", Kind::path))
        >
            if let Some(kind) = collection {
                <div class="heading-art">(icon(cx, Some(kind)))</div>
            } else if overview {
                <div class="hero-record" aria-hidden="true"></div>
            }
            <div class="heading-copy">
                <h1>(title)</h1>
                if overview {
                    <p class="heading-description">
                        "Explore artists, albums and tracks, down to the credits and lyrics."
                    </p>
                }
            </div>
        </section>
    }
    .boxed()
}

pub(super) fn layout<'a>(
    cx: &'a Cx,
    title: String,
    selected: Option<Kind>,
    query: String,
    content: BoxView<'a>,
) -> impl View + 'a {
    view! {
        <!DOCTYPE html>
        <html lang="en">
            <head>
                <meta charset="utf-8">
                <meta name="viewport" content="width=device-width, initial-scale=1">
                <title>
                    (title.clone())
                    " · Concentus"
                </title>
                <link rel="stylesheet" href="/assets/catalog.css">
                <link rel="icon" type="image/svg+xml" href="/favicon.svg">
                <script src="/assets/tag-details.js" defer=(true)></script>
            </head>
            <body>
                <a class="skip" href="#main">"Skip to content"</a>
                <aside class="rail">
                    <a class="brand" href="/">
                        <span class="mark">(icon(cx, None))</span>
                        "Concentus"
                    </a>
                    <nav class="tabs" aria-label="Collections">
                        <a
                            href="/"
                            aria-current=(if selected.is_none() && query.is_empty() {
                                "page"
                            } else {
                                "false"
                            })
                        >
                            (icon(cx, None))
                            <span>"Overview"</span>
                        </a>
                        for kind in [Kind::Artist, Kind::Album, Kind::Track, Kind::Tag] {
                            <a
                                href=(format!("/?kind={}", kind.path()))
                                data-kind=(kind.path())
                                aria-current=(if selected == Some(kind) {
                                    "page"
                                } else {
                                    "false"
                                })
                            >
                                (icon(cx, Some(kind)))
                                <span>(kind.plural())</span>
                            </a>
                        }
                    </nav>
                </aside>
                <div class="shell">
                    <header class="topbar">
                        <form class="search" action="/" method="get" role="search">
                            <label class="sr-only" for="query">
                                "Search the catalog"
                            </label>
                            <input
                                id="query"
                                type="search"
                                name="q"
                                value=(query.clone())
                                placeholder="Search the catalog"
                                maxlength="160"
                                required=(true)
                            >
                            <label class="sr-only" for="category">"Collection"</label>
                            <select id="category" name="kind">
                                <option value="">"All types"</option>
                                for kind in Kind::ALL {
                                    <option
                                        value=(kind.path())
                                        selected=(selected == Some(kind))
                                    >
                                        (kind.plural())
                                    </option>
                                }
                            </select>
                            <button type="submit">"Search"</button>
                        </form>
                    </header>
                    <main id="main">(content)</main>
                </div>
            </body>
        </html>
    }
}
