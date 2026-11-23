//! Keyless release artwork: <https://musicbrainz.org/doc/Cover_Art_Archive/API>.
//!
//! The full image index can be used later to import alternate covers and scans.

pub const RELEASE_GROUP_PROVIDER: &str = "MusicBrainz release group";

pub fn front(id: &str, release_group: bool) -> Option<String> {
    let id = id.parse::<uuid::Uuid>().ok()?;
    let collection = if release_group {
        "release-group"
    } else {
        "release"
    };
    Some(format!(
        "https://coverartarchive.org/{collection}/{id}/front-500"
    ))
}
