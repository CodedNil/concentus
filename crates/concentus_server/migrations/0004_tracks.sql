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
    valence INTEGER,
    record_collection TEXT GENERATED ALWAYS AS ('tracks') VIRTUAL,
    FOREIGN KEY (id, record_collection) REFERENCES records(id, collection) ON DELETE CASCADE DEFERRABLE INITIALLY DEFERRED
) STRICT;
CREATE INDEX tracks_title ON tracks (trim(title) COLLATE NOCASE);
CREATE INDEX tracks_album ON tracks (album_id);

CREATE TABLE track_languages (
    owner_id TEXT NOT NULL REFERENCES tracks(id) ON DELETE CASCADE DEFERRABLE INITIALLY DEFERRED,
    position INTEGER NOT NULL,
    language TEXT NOT NULL,
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
    PRIMARY KEY (owner_id, position),
    CHECK (
        (from_start_ms IS NULL AND from_end_ms IS NULL)
        OR (from_start_ms IS NOT NULL AND from_end_ms IS NOT NULL AND from_start_ms >= 0 AND from_end_ms >= from_start_ms)
    ),
    CHECK (
        (to_start_ms IS NULL AND to_end_ms IS NULL)
        OR (to_start_ms IS NOT NULL AND to_end_ms IS NOT NULL AND to_start_ms >= 0 AND to_end_ms >= to_start_ms)
    )
) STRICT, WITHOUT ROWID;

CREATE TABLE track_lyrics (
    track_id TEXT NOT NULL REFERENCES tracks(id) ON DELETE CASCADE DEFERRABLE INITIALLY DEFERRED,
    language TEXT COLLATE NOCASE NOT NULL,
    instrumental INTEGER NOT NULL CHECK (instrumental IN (0, 1)),
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
    start_ms INTEGER CHECK (start_ms >= 0),
    end_ms INTEGER CHECK (end_ms >= 0),
    PRIMARY KEY (owner_id, position),
    CHECK (end_ms >= start_ms)
) STRICT, WITHOUT ROWID;

-- Catalog full-text search
CREATE TRIGGER search_tracks_insert AFTER INSERT ON tracks BEGIN
    INSERT INTO catalog_search (record_id, collection, title) VALUES (new.id, 'tracks', new.title);
END;
CREATE TRIGGER search_tracks_update AFTER UPDATE OF id, title ON tracks
WHEN old.id IS NOT new.id OR old.title IS NOT new.title BEGIN
    DELETE FROM catalog_search WHERE record_id = old.id AND collection = 'tracks';
    INSERT INTO catalog_search (record_id, collection, title) VALUES (new.id, 'tracks', new.title);
END;
CREATE TRIGGER search_tracks_delete AFTER DELETE ON tracks BEGIN
    DELETE FROM catalog_search WHERE record_id = old.id AND collection = 'tracks';
END;
