use topcoat::{
    Result,
    context::Cx,
    router::{Body, response::Response, route},
};

const CATALOG_CSS: &str = concat!(
    include_str!("styles/base.css"),
    include_str!("styles/shell.css"),
    include_str!("styles/heroes.css"),
    include_str!("styles/components.css"),
    include_str!("styles/browse.css"),
    include_str!("styles/records.css"),
    include_str!("styles/editor.css"),
    include_str!("tags/board.css"),
    include_str!("tags/details.css"),
);

#[route(GET "/assets/catalog.css")]
pub(super) async fn stylesheet(cx: &Cx) -> Result<Response> {
    let _ = cx;
    Ok(Response::builder()
        .header("content-type", "text/css; charset=utf-8")
        .body(Body::from(CATALOG_CSS))?)
}

#[route(GET "/assets/editor.js")]
pub(super) async fn editor_script(cx: &Cx) -> Result<Response> {
    let _ = cx;
    Ok(Response::builder()
        .header("content-type", "text/javascript; charset=utf-8")
        .body(Body::from(include_str!("editor.js")))?)
}

#[route(GET "/favicon.svg")]
pub(super) async fn favicon(cx: &Cx) -> Result<Response> {
    let _ = cx;
    Ok(Response::builder()
        .header("content-type", "image/svg+xml")
        .body(Body::from(include_str!("favicon.svg")))?)
}

#[route(GET "/assets/tags.js")]
pub(super) async fn tag_script(cx: &Cx) -> Result<Response> {
    let _ = cx;
    Ok(Response::builder()
        .header("content-type", "text/javascript; charset=utf-8")
        .body(Body::from(include_str!("tags/board.js")))?)
}

#[route(GET "/assets/browse.js")]
pub(super) async fn browse_script(cx: &Cx) -> Result<Response> {
    let _ = cx;
    Ok(Response::builder()
        .header("content-type", "text/javascript; charset=utf-8")
        .body(Body::from(include_str!("browse.js")))?)
}

#[route(GET "/assets/save-edits.js")]
pub(super) async fn save_edits_script(cx: &Cx) -> Result<Response> {
    let _ = cx;
    Ok(Response::builder()
        .header("content-type", "text/javascript; charset=utf-8")
        .body(Body::from(include_str!("save-edits.js")))?)
}
#[route(GET "/assets/tag-details.js")]
pub(super) async fn tag_details_script(cx: &Cx) -> Result<Response> {
    let _ = cx;
    Ok(Response::builder()
        .header("content-type", "text/javascript; charset=utf-8")
        .body(Body::from(include_str!("tags/details.js")))?)
}
