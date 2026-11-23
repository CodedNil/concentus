CREATE TABLE artists (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    sort_name TEXT,
    kind TEXT NOT NULL,
    biography TEXT,
    begin_date TEXT,
    end_date TEXT,
    origin TEXT,
    theme_background TEXT,
    theme_foreground TEXT,
    theme_accent TEXT,
    theme_font_family TEXT,
    theme_font_url TEXT
) STRICT;
CREATE INDEX artists_name ON artists (trim(name) COLLATE NOCASE);

CREATE TABLE artist_artwork (
    id INTEGER PRIMARY KEY,
    owner_id TEXT NOT NULL REFERENCES artists(id) ON DELETE CASCADE DEFERRABLE INITIALLY DEFERRED,
    position INTEGER NOT NULL,
    kind TEXT NOT NULL,
    blurhash TEXT,
    explicit INTEGER NOT NULL,
    UNIQUE (owner_id, position)
) STRICT;

CREATE TABLE artist_artwork_renditions (
    owner_id INTEGER NOT NULL REFERENCES artist_artwork(id) ON DELETE CASCADE DEFERRABLE INITIALLY DEFERRED,
    position INTEGER NOT NULL,
    url TEXT NOT NULL,
    width INTEGER NOT NULL,
    height INTEGER NOT NULL,
    codec TEXT,
    duration_ms INTEGER,
    PRIMARY KEY (owner_id, position)
) STRICT, WITHOUT ROWID;

CREATE TABLE artist_artwork_colors (
    owner_id INTEGER NOT NULL REFERENCES artist_artwork(id) ON DELETE CASCADE DEFERRABLE INITIALLY DEFERRED,
    position INTEGER NOT NULL,
    color TEXT NOT NULL,
    PRIMARY KEY (owner_id, position)
) STRICT, WITHOUT ROWID;

CREATE TABLE artist_members (
    id INTEGER PRIMARY KEY,
    owner_id TEXT NOT NULL REFERENCES artists(id) ON DELETE CASCADE DEFERRABLE INITIALLY DEFERRED,
    position INTEGER NOT NULL,
    artist_id TEXT NOT NULL REFERENCES artists(id) DEFERRABLE INITIALLY DEFERRED,
    joined_date TEXT,
    left_date TEXT,
    UNIQUE (owner_id, position)
) STRICT;

CREATE TABLE member_roles (
    owner_id INTEGER NOT NULL REFERENCES artist_members(id) ON DELETE CASCADE DEFERRABLE INITIALLY DEFERRED,
    position INTEGER NOT NULL,
    role TEXT NOT NULL,
    role_detail TEXT,
    PRIMARY KEY (owner_id, position)
) STRICT, WITHOUT ROWID;

CREATE TABLE artist_tags (
    owner_id TEXT NOT NULL REFERENCES artists(id) ON DELETE CASCADE DEFERRABLE INITIALLY DEFERRED,
    position INTEGER NOT NULL,
    tag_id TEXT NOT NULL REFERENCES tags(id) DEFERRABLE INITIALLY DEFERRED,
    weight INTEGER NOT NULL,
    PRIMARY KEY (owner_id, position)
) STRICT, WITHOUT ROWID;

CREATE TABLE artist_external_ids (
    owner_id TEXT NOT NULL REFERENCES artists(id) ON DELETE CASCADE DEFERRABLE INITIALLY DEFERRED,
    provider TEXT NOT NULL,
    value TEXT NOT NULL,
    PRIMARY KEY (owner_id, provider, value)
) STRICT, WITHOUT ROWID;
CREATE INDEX artist_external_ids_provider ON artist_external_ids (provider, value, owner_id);
