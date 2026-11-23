mod api;
mod app;
mod catalog;
mod lookup;
mod records;
mod store;

use std::{env, io, path::PathBuf};

use directories::ProjectDirs;
use store::Store;

#[tokio::main]
async fn main() -> topcoat::Result<()> {
    let database = database_path()?;
    let store = Store::open(&database).await?;
    eprintln!("Concentus database: {}", database.display());
    Ok(topcoat::start(app::router(store)).await?)
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
