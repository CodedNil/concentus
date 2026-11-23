use concentus_types::{Album, Artist, Credit, Id, Role, Track};

#[derive(Debug)]
pub struct Catalog {
    /// All artists.
    pub artists: Vec<Artist>,
    /// All albums.
    pub albums: Vec<Album>,
    /// All tracks.
    pub tracks: Vec<Track>,
}

impl Catalog {
    /// The names of the billed primary and featured artists, in credit order.
    pub fn artist_names(&self, credits: &[Credit]) -> String {
        credits
            .iter()
            .filter(|c| matches!(c.role, Role::Primary | Role::Featured))
            .filter_map(|c| self.artists.iter().find(|a| a.id == c.artist_id))
            .map(|a| a.name.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    }

    pub fn album_title(&self, id: Option<Id<Album>>) -> &str {
        self.albums
            .iter()
            .find(|a| Some(a.id) == id)
            .map_or("No album", |a| a.title.as_str())
    }

    /// The `(disc, track)` position of a track on its primary album.
    pub fn position(&self, track: &Track) -> Option<(u16, u16)> {
        self.albums
            .iter()
            .find(|a| Some(a.id) == track.album_id())?
            .position(track.id)
    }

    pub fn track_title(&self, id: Id<Track>) -> &str {
        self.tracks
            .iter()
            .find(|t| t.id == id)
            .map_or("Unknown track", |t| t.title.as_str())
    }
}
