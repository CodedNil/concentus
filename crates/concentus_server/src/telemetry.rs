use std::time::Instant;
use topcoat::{
    context::Cx,
    router::{Body, LayerFuture, Next, request},
};
use tracing::{debug, info, warn};
use tracing_subscriber::EnvFilter;

pub fn init() {
    let filter = EnvFilter::builder()
        .with_default_directive(tracing::Level::WARN.into())
        .from_env_lossy();
    let filter = if std::env::var_os("RUST_LOG").is_none() {
        filter.add_directive(
            "concentus_server=info"
                .parse()
                .expect("valid log directive"),
        )
    } else {
        filter
    };
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .with_writer(std::io::stderr)
        .init();
}

pub fn request<'a>(cx: &'a Cx, body: Body, next: Next<'a>) -> LayerFuture<'a> {
    Box::pin(async move {
        let start = Instant::now();
        let result = next.run(cx, body).await;
        let method = request::method(cx);
        let path = request::uri(cx).path();
        let elapsed_ms = start.elapsed().as_millis();
        if let Ok(response) = &result {
            let status = response.status().as_u16();
            if status >= 500 {
                warn!(%method, path, status, elapsed_ms, "HTTP response failed");
            } else if path.starts_with("/assets/") || path == "/favicon.svg" {
                debug!(%method, path, status, elapsed_ms, "Asset response ready");
            } else {
                info!(%method, path, status, elapsed_ms, "HTTP response ready");
            }
        } else {
            debug!(%method, path, elapsed_ms, "HTTP handler returned an error");
        }
        result
    })
}
