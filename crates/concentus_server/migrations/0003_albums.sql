-- Albums
CREATE TABLE albums (
    id TEXT PRIMARY KEY,
    title TEXT NOT NULL,
    sort_title TEXT,
    edition_name TEXT,
    edition_of TEXT REFERENCES albums(id) DEFERRABLE INITIALLY DEFERRED,
    album_type TEXT,
    soundtrack_source TEXT,
    release_date TEXT,
    label TEXT,
    copyright TEXT,
    description TEXT,
    theme_background TEXT,
    theme_foreground TEXT,
    theme_accent TEXT,
    record_collection TEXT GENERATED ALWAYS AS ('albums') VIRTUAL,
    FOREIGN KEY (id, record_collection) REFERENCES records(id, collection) ON DELETE CASCADE DEFERRABLE INITIALLY DEFERRED
) STRICT;
CREATE INDEX albums_title ON albums (trim(title) COLLATE NOCASE);

CREATE TABLE album_discs (
    id INTEGER PRIMARY KEY,
    owner_id TEXT NOT NULL REFERENCES albums(id) ON DELETE CASCADE DEFERRABLE INITIALLY DEFERRED,
    position INTEGER NOT NULL,
    title TEXT,
    UNIQUE (owner_id, position)
) STRICT;

CREATE TABLE disc_tracks (
    owner_id INTEGER NOT NULL REFERENCES album_discs(id) ON DELETE CASCADE DEFERRABLE INITIALLY DEFERRED,
    position INTEGER NOT NULL,
    track_id TEXT NOT NULL REFERENCES tracks(id) DEFERRABLE INITIALLY DEFERRED,
    PRIMARY KEY (owner_id, position)
) STRICT, WITHOUT ROWID;
CREATE INDEX disc_tracks_track ON disc_tracks (track_id, owner_id);
