use directories::ProjectDirs;
use std::{env, io, path::PathBuf};
use store::Store;

mod api;
mod catalog;
mod enrichment;
mod lookup;
mod records;
mod store;
mod web;

#[tokio::main]
async fn main() -> topcoat::Result<()> {
    let database = database_path()?;
    let store = Store::open(&database).await?;
    eprintln!("Concentus database: {}", database.display());
    let router = topcoat::router::Router::builder()
        .with(store)
        .with(enrichment::Enrichment::new()?);
    Ok(topcoat::start(api::register(web::register(router)).build()).await?)
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
