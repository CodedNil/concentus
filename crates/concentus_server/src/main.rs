use directories::ProjectDirs;
use std::{env, io, path::PathBuf};
use store::Store;

mod access;
mod api;
mod catalog;
mod editor;
mod enrichment;
mod lookup;
mod records;
mod store;
mod telemetry;
mod web;

#[tokio::main]
async fn main() -> topcoat::Result<()> {
    telemetry::init();
    let database = database_path()?;
    tracing::info!(database = %database.display(), "Opening catalog database");
    let store = Store::open(&database).await?;
    tracing::info!("Catalog database ready");
    let enrichment = enrichment::Enrichment::new()?;
    enrichment.clone().worker(store.clone());
    let (stop_access, stop_signal) = tokio::sync::oneshot::channel();
    let access_worker = access::AccessLog::worker(store.clone(), stop_signal);
    let router = topcoat::router::Router::builder()
        .layer(topcoat::router::LayerFn::new(
            None::<&str>,
            telemetry::request,
        ))
        .with(store.clone())
        .with(enrichment);
    tracing::info!("Starting HTTP server");
    let result = topcoat::start(api::register(web::register(router)).build()).await;
    let _ = stop_access.send(());
    let _ = access_worker.await;
    store.flush_access().await;
    Ok(result?)
}

fn database_path() -> io::Result<PathBuf> {
    if let Some(path) = env::var_os("CONCENTUS_DATABASE").filter(|path| !path.is_empty()) {
        return Ok(path.into());
    }

    let dirs = ProjectDirs::from("", "", "concentus").ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::NotFound,
            "No local state directory. Set CONCENTUS_DATABASE to a file path.",
        )
    })?;
    Ok(dirs
        .state_dir()
        .unwrap_or_else(|| dirs.data_local_dir())
        .join("catalog.sqlite3"))
}
