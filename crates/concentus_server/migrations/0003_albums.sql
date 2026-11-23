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
    theme_accent TEXT
) STRICT;
CREATE INDEX albums_title ON albums (trim(title) COLLATE NOCASE);

CREATE TABLE album_artwork (
    id INTEGER PRIMARY KEY,
    owner_id TEXT NOT NULL REFERENCES albums(id) ON DELETE CASCADE DEFERRABLE INITIALLY DEFERRED,
    position INTEGER NOT NULL,
    kind TEXT NOT NULL,
    blurhash TEXT,
    explicit INTEGER NOT NULL,
    UNIQUE (owner_id, position)
) STRICT;

CREATE TABLE album_artwork_renditions (
    owner_id INTEGER NOT NULL REFERENCES album_artwork(id) ON DELETE CASCADE DEFERRABLE INITIALLY DEFERRED,
    position INTEGER NOT NULL,
    url TEXT NOT NULL,
    width INTEGER NOT NULL,
    height INTEGER NOT NULL,
    codec TEXT,
    duration_ms INTEGER,
    PRIMARY KEY (owner_id, position)
) STRICT, WITHOUT ROWID;

CREATE TABLE album_artwork_colors (
    owner_id INTEGER NOT NULL REFERENCES album_artwork(id) ON DELETE CASCADE DEFERRABLE INITIALLY DEFERRED,
    position INTEGER NOT NULL,
    color TEXT NOT NULL,
    PRIMARY KEY (owner_id, position)
) STRICT, WITHOUT ROWID;

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

CREATE TABLE album_credits (
    owner_id TEXT NOT NULL REFERENCES albums(id) ON DELETE CASCADE DEFERRABLE INITIALLY DEFERRED,
    position INTEGER NOT NULL,
    artist_id TEXT NOT NULL REFERENCES artists(id) DEFERRABLE INITIALLY DEFERRED,
    role TEXT NOT NULL,
    role_detail TEXT,
    PRIMARY KEY (owner_id, position)
) STRICT, WITHOUT ROWID;
CREATE INDEX album_credits_artist ON album_credits (artist_id, role, owner_id);

CREATE TABLE album_tags (
    owner_id TEXT NOT NULL REFERENCES albums(id) ON DELETE CASCADE DEFERRABLE INITIALLY DEFERRED,
    position INTEGER NOT NULL,
    tag_id TEXT NOT NULL REFERENCES tags(id) DEFERRABLE INITIALLY DEFERRED,
    weight INTEGER NOT NULL,
    PRIMARY KEY (owner_id, position)
) STRICT, WITHOUT ROWID;

CREATE TABLE album_external_ids (
    owner_id TEXT NOT NULL REFERENCES albums(id) ON DELETE CASCADE DEFERRABLE INITIALLY DEFERRED,
    provider TEXT NOT NULL,
    value TEXT NOT NULL,
    PRIMARY KEY (owner_id, provider, value)
) STRICT, WITHOUT ROWID;
CREATE INDEX album_external_ids_provider ON album_external_ids (provider, value, owner_id);
