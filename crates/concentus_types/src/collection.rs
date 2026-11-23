use crate::{Album, Artist, Tag, Track};
use serde::{Serialize, de::DeserializeOwned};

/// A catalog record addressed through a named collection in the API and database.
pub trait Collection: Serialize + DeserializeOwned {
    const NAME: &'static str;
}

impl Collection for Artist {
    const NAME: &'static str = "artists";
}
impl Collection for Album {
    const NAME: &'static str = "albums";
}
impl Collection for Track {
    const NAME: &'static str = "tracks";
}
impl Collection for Tag {
    const NAME: &'static str = "tags";
}
