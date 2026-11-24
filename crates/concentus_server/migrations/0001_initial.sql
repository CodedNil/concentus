-- Tags
CREATE TABLE tags (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    kind TEXT NOT NULL,
    parent TEXT REFERENCES tags(id) DEFERRABLE INITIALLY DEFERRED
) STRICT;
CREATE INDEX tags_name ON tags (trim(name) COLLATE NOCASE);

CREATE TABLE tag_aliases (
    owner_id TEXT NOT NULL REFERENCES tags(id) ON DELETE CASCADE DEFERRABLE INITIALLY DEFERRED,
    position INTEGER NOT NULL,
    alias TEXT NOT NULL,
    PRIMARY KEY (owner_id, position)
) STRICT, WITHOUT ROWID;

-- Artists
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

-- Tracks
CREATE TABLE tracks (
    id TEXT PRIMARY KEY,
    title TEXT NOT NULL,
    duration_ms INTEGER NOT NULL,
    version TEXT,
    album_id TEXT REFERENCES albums(id) DEFERRABLE INITIALLY DEFERRED,
    release_date TEXT,
    rating TEXT,
    about TEXT,
    music_video TEXT,
    tempo_bpm REAL,
    key TEXT,
    mode TEXT,
    time_signature INTEGER,
    loudness_lufs REAL,
    peak REAL,
    acousticness INTEGER,
    danceability INTEGER,
    energy INTEGER,
    instrumentalness INTEGER,
    liveness INTEGER,
    speechiness INTEGER,
    valence INTEGER
) STRICT;
CREATE INDEX tracks_title ON tracks (trim(title) COLLATE NOCASE);
CREATE INDEX tracks_album ON tracks (album_id);

CREATE TABLE track_artwork (
    id INTEGER PRIMARY KEY,
    owner_id TEXT NOT NULL REFERENCES tracks(id) ON DELETE CASCADE DEFERRABLE INITIALLY DEFERRED,
    position INTEGER NOT NULL,
    kind TEXT NOT NULL,
    blurhash TEXT,
    explicit INTEGER NOT NULL,
    UNIQUE (owner_id, position)
) STRICT;

CREATE TABLE track_artwork_renditions (
    owner_id INTEGER NOT NULL REFERENCES track_artwork(id) ON DELETE CASCADE DEFERRABLE INITIALLY DEFERRED,
    position INTEGER NOT NULL,
    url TEXT NOT NULL,
    width INTEGER NOT NULL,
    height INTEGER NOT NULL,
    codec TEXT,
    duration_ms INTEGER,
    PRIMARY KEY (owner_id, position)
) STRICT, WITHOUT ROWID;

CREATE TABLE track_artwork_colors (
    owner_id INTEGER NOT NULL REFERENCES track_artwork(id) ON DELETE CASCADE DEFERRABLE INITIALLY DEFERRED,
    position INTEGER NOT NULL,
    color TEXT NOT NULL,
    PRIMARY KEY (owner_id, position)
) STRICT, WITHOUT ROWID;

CREATE TABLE track_credits (
    owner_id TEXT NOT NULL REFERENCES tracks(id) ON DELETE CASCADE DEFERRABLE INITIALLY DEFERRED,
    position INTEGER NOT NULL,
    artist_id TEXT NOT NULL REFERENCES artists(id) DEFERRABLE INITIALLY DEFERRED,
    role TEXT NOT NULL,
    role_detail TEXT,
    PRIMARY KEY (owner_id, position)
) STRICT, WITHOUT ROWID;
CREATE INDEX track_credits_artist ON track_credits (artist_id, role, owner_id);

CREATE TABLE track_languages (
    owner_id TEXT NOT NULL REFERENCES tracks(id) ON DELETE CASCADE DEFERRABLE INITIALLY DEFERRED,
    position INTEGER NOT NULL,
    language TEXT NOT NULL,
    PRIMARY KEY (owner_id, position)
) STRICT, WITHOUT ROWID;

CREATE TABLE track_tags (
    owner_id TEXT NOT NULL REFERENCES tracks(id) ON DELETE CASCADE DEFERRABLE INITIALLY DEFERRED,
    position INTEGER NOT NULL,
    tag_id TEXT NOT NULL REFERENCES tags(id) DEFERRABLE INITIALLY DEFERRED,
    weight INTEGER NOT NULL,
    PRIMARY KEY (owner_id, position)
) STRICT, WITHOUT ROWID;

CREATE TABLE track_derivations (
    owner_id TEXT NOT NULL REFERENCES tracks(id) ON DELETE CASCADE DEFERRABLE INITIALLY DEFERRED,
    position INTEGER NOT NULL,
    track_id TEXT NOT NULL REFERENCES tracks(id) DEFERRABLE INITIALLY DEFERRED,
    kind TEXT NOT NULL,
    from_start_ms INTEGER,
    from_end_ms INTEGER,
    to_start_ms INTEGER,
    to_end_ms INTEGER,
    PRIMARY KEY (owner_id, position)
) STRICT, WITHOUT ROWID;

CREATE TABLE track_external_ids (
    owner_id TEXT NOT NULL REFERENCES tracks(id) ON DELETE CASCADE DEFERRABLE INITIALLY DEFERRED,
    provider TEXT NOT NULL,
    value TEXT NOT NULL,
    PRIMARY KEY (owner_id, provider, value)
) STRICT, WITHOUT ROWID;
CREATE INDEX track_external_ids_provider ON track_external_ids (provider, value, owner_id);

CREATE TABLE track_lyrics (
    track_id TEXT NOT NULL REFERENCES tracks(id) ON DELETE CASCADE DEFERRABLE INITIALLY DEFERRED,
    language TEXT COLLATE NOCASE NOT NULL,
    instrumental INTEGER NOT NULL,
    PRIMARY KEY (track_id, language)
) STRICT, WITHOUT ROWID;

CREATE TABLE lyrics_channels (
    id INTEGER PRIMARY KEY,
    track_id TEXT NOT NULL,
    language TEXT COLLATE NOCASE NOT NULL,
    position INTEGER NOT NULL,
    artist_id TEXT REFERENCES artists(id) DEFERRABLE INITIALLY DEFERRED,
    kind TEXT NOT NULL,
    UNIQUE (track_id, language, position),
    FOREIGN KEY (track_id, language) REFERENCES track_lyrics(track_id, language)
        ON DELETE CASCADE DEFERRABLE INITIALLY DEFERRED
) STRICT;

CREATE TABLE channel_spans (
    owner_id INTEGER NOT NULL REFERENCES lyrics_channels(id) ON DELETE CASCADE DEFERRABLE INITIALLY DEFERRED,
    position INTEGER NOT NULL,
    text TEXT NOT NULL,
    start_ms INTEGER,
    end_ms INTEGER,
    PRIMARY KEY (owner_id, position)
) STRICT, WITHOUT ROWID;

-- Enrichment queue and revision history
CREATE TABLE enrichment_needed (
    collection TEXT NOT NULL,
    record_id TEXT NOT NULL,
    field TEXT NOT NULL,
    due_at INTEGER NOT NULL,
    generation INTEGER NOT NULL,
    refresh INTEGER NOT NULL,
    PRIMARY KEY (collection, record_id, field)
) STRICT, WITHOUT ROWID;
CREATE INDEX enrichment_due ON enrichment_needed (due_at);

CREATE TABLE record_history (
    collection TEXT NOT NULL,
    record_id TEXT NOT NULL,
    field TEXT NOT NULL,
    minute INTEGER NOT NULL,
    old_value TEXT NOT NULL,
    new_value TEXT NOT NULL,
    PRIMARY KEY (collection, record_id, field, minute)
) STRICT, WITHOUT ROWID;

