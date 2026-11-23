use crate::store::{Store, store};
use concentus_types::{Language, Lyrics, Track};
use topcoat::{
    Result,
    context::Cx,
    metadata::Metadata,
    router::{Body, Router, page, response::Response, route},
    view::{View, view},
};

pub fn router(store: Store) -> Router {
    crate::api::register(Router::builder().with(store).route(stylesheet).page(home)).build()
}

#[route(GET "/assets/catalog.css")]
async fn stylesheet(cx: &Cx) -> Result<Response> {
    const CSS: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/tailwind.css"));
    let _ = cx;
    Ok(Response::builder()
        .header("content-type", "text/css; charset=utf-8")
        .body(Body::from(CSS))?)
}

#[page(
    "/",
    with(Metadata::new()
        .title("Concentus · Music metadata")
        .description("Concentus music metadata: tracks, albums, artists and lyrics.")
        .viewport("width=device-width, initial-scale=1"))
)]
async fn home(cx: &Cx) -> Result<impl View> {
    let catalog = store(cx).catalog().await?;
    Ok(view! {
        <!DOCTYPE html>
        <html lang="en">
            <head>
                topcoat::metadata::tags()
                <link rel="stylesheet" href="/assets/catalog.css">
            </head>
            <body>
                <header>
                    <a class="brand" href="/">
                        "concentus"
                        <span>"."</span>
                    </a>
                    <p>"Music, described."</p>
                    <span class="badge">"Sample catalog"</span>
                </header>
                <main>
                    <div class="intro">
                        <span class="eyebrow">"Music metadata provider"</span>
                        <h1>"Every record has a story."</h1>
                        <p>
                            "Explore tracks, albums, artists and lyrics in one catalog."
                        </p>
                    </div>
                    <nav aria-label="Catalog sections">
                        <a href="#tracks">
                            "Tracks · "
                            (catalog.tracks.len())
                        </a>
                        <a href="#albums">
                            "Albums · "
                            (catalog.albums.len())
                        </a>
                        <a href="#artists">
                            "Artists · "
                            (catalog.artists.len())
                        </a>
                    </nav>
                    <section id="tracks" aria-labelledby="tracks-heading">
                        <div class="section-heading">
                            <h2 id="tracks-heading">"Tracks"</h2>
                        </div>
                        <div class="grid">
                            for track in &catalog.tracks {
                                <article class="card">
                                    <span class="eyebrow">"Track"</span>
                                    <h3>(track.title.clone())</h3>
                                    <p class="muted">
                                        <a href="#artists">
                                            (catalog.artist_names(&track.credits))
                                        </a>
                                        " · "
                                        <a href="#albums">
                                            (catalog.album_title(track.album_id).to_owned())
                                        </a>
                                    </p>
                                    <dl>
                                        <dt>"Duration"</dt>
                                        <dd>
                                            (format!(
                                                "{}:{:02}",
                                                track.duration_ms / 60_000,
                                                track.duration_ms / 1_000 % 60,
                                            ))
                                        </dd>
                                        <dt>"Disc / track"</dt>
                                        <dd>
                                            (catalog
                                                .position(track)
                                                .map_or_else(
                                                    || "1 / 1".to_owned(),
                                                    |(disc, number)| format!("{disc} / {number}"),
                                                ))
                                        </dd>
                                    </dl>
                                    for (language, lyrics) in lyrics_by_language(track) {
                                        <details>
                                            <summary>(format!("Lyrics · {language}"))</summary>
                                            if lyrics.instrumental {
                                                <p>"Instrumental"</p>
                                            } else {
                                                <pre class="lyrics">
                                                    (lyrics.channels.iter().flat_map(|channel| &channel.spans).map(|span| span.text.as_str()).collect::<String>())
                                                </pre>
                                            }
                                        </details>
                                    }
                                    <span class="id">
                                        (track.id.to_string())
                                    </span>
                                </article>
                            }
                        </div>
                    </section>
                    <section id="albums" aria-labelledby="albums-heading">
                        <div class="section-heading">
                            <h2 id="albums-heading">"Albums"</h2>
                        </div>
                        <div class="grid">
                            for album in &catalog.albums {
                                <article class="card">
                                    <div class="art" aria-hidden="true">"◎"</div>
                                    <h3>(album.title.clone())</h3>
                                    <p class="muted">
                                        (catalog.artist_names(&album.credits))
                                        " · "
                                        (album
                                            .release_date
                                            .map(|d| d.to_string())
                                            .unwrap_or_default())
                                    </p>
                                    <ol>
                                        for id in album.track_ids() {
                                            <li>(catalog.track_title(id).to_owned())</li>
                                        }
                                    </ol>
                                    <span class="id">
                                        (album.id.to_string())
                                    </span>
                                </article>
                            }
                        </div>
                    </section>
                    <section id="artists" aria-labelledby="artists-heading">
                        <div class="section-heading">
                            <h2 id="artists-heading">"Artists"</h2>
                        </div>
                        <div class="grid">
                            for artist in &catalog.artists {
                                <article class="card">
                                    <span class="eyebrow">"Artist"</span>
                                    <h3>(artist.name.clone())</h3>
                                    <p class="muted">
                                        (artist.biography.clone().unwrap_or_default())
                                    </p>
                                    <span class="id">
                                        (artist.id.to_string())
                                    </span>
                                </article>
                            }
                        </div>
                    </section>
                </main>
                <footer>"Concentus · A home for music metadata"</footer>
            </body>
        </html>
    })
}

fn lyrics_by_language(track: &Track) -> Vec<(&Language, &Lyrics)> {
    let mut lyrics: Vec<_> = track.lyrics.iter().collect();
    lyrics.sort_unstable_by_key(|(language, _)| *language);
    lyrics
}
