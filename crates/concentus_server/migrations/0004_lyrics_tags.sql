CREATE TABLE track_lyrics (
    track_id TEXT NOT NULL REFERENCES tracks(id) ON DELETE CASCADE DEFERRABLE INITIALLY DEFERRED,
    language TEXT COLLATE NOCASE NOT NULL CHECK (length(language) > 0),
    instrumental INTEGER NOT NULL CHECK (instrumental IN (0, 1)),
    PRIMARY KEY (track_id, language)
) STRICT, WITHOUT ROWID;

CREATE TABLE lyrics_channels (
    id INTEGER PRIMARY KEY,
    track_id TEXT NOT NULL,
    language TEXT COLLATE NOCASE NOT NULL,
    position INTEGER NOT NULL CHECK (position >= 0),
    artist_id TEXT REFERENCES artists(id) DEFERRABLE INITIALLY DEFERRED,
    kind TEXT NOT NULL CHECK (kind IN ('Lead', 'Backing', 'Harmony', 'AdLib', 'Spoken', 'Crowd', 'SoundCue')),
    UNIQUE (track_id, language, position),
    FOREIGN KEY (track_id, language) REFERENCES track_lyrics(track_id, language)
        ON DELETE CASCADE DEFERRABLE INITIALLY DEFERRED
) STRICT;

CREATE TABLE channel_spans (
    owner_id INTEGER NOT NULL REFERENCES lyrics_channels(id) ON DELETE CASCADE DEFERRABLE INITIALLY DEFERRED,
    position INTEGER NOT NULL CHECK (position >= 0),
    text TEXT NOT NULL,
    start_ms INTEGER CHECK (start_ms BETWEEN 0 AND 4294967295),
    end_ms INTEGER CHECK (end_ms BETWEEN 0 AND 4294967295),
    PRIMARY KEY (owner_id, position)
) STRICT, WITHOUT ROWID;

CREATE TABLE tag_aliases (
    owner_id TEXT NOT NULL REFERENCES tags(id) ON DELETE CASCADE DEFERRABLE INITIALLY DEFERRED,
    position INTEGER NOT NULL CHECK (position >= 0),
    alias TEXT NOT NULL,
    PRIMARY KEY (owner_id, position)
) STRICT, WITHOUT ROWID;
