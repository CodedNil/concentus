-- Collection IDs use canonical lowercase UUID text. References are deferred for cyclic imports.
CREATE TABLE artists (
    id TEXT PRIMARY KEY CHECK (
        length(id) = 36 AND substr(id, 9, 1) = '-' AND substr(id, 14, 1) = '-'
        AND substr(id, 19, 1) = '-' AND substr(id, 24, 1) = '-'
        AND length(replace(id, '-', '')) = 32 AND id NOT GLOB '*[^0-9a-f-]*'
    ),
    name TEXT NOT NULL,
    sort_name TEXT,
    kind TEXT NOT NULL CHECK (kind IN ('Person', 'Group', 'Unknown')),
    biography TEXT,
    begin TEXT CHECK (begin GLOB '[0-9][0-9][0-9][0-9]' OR begin GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]' OR begin GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]'),
    "end" TEXT CHECK ("end" GLOB '[0-9][0-9][0-9][0-9]' OR "end" GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]' OR "end" GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]'),
    origin TEXT,
    visuals_id INTEGER REFERENCES visuals(id) DEFERRABLE INITIALLY DEFERRED
) STRICT;
CREATE INDEX artists_name ON artists (trim(name) COLLATE NOCASE);

CREATE TABLE albums (
    id TEXT PRIMARY KEY CHECK (
        length(id) = 36 AND substr(id, 9, 1) = '-' AND substr(id, 14, 1) = '-'
        AND substr(id, 19, 1) = '-' AND substr(id, 24, 1) = '-'
        AND length(replace(id, '-', '')) = 32 AND id NOT GLOB '*[^0-9a-f-]*'
    ),
    title TEXT NOT NULL,
    sort_title TEXT,
    edition_name TEXT,
    edition_of TEXT REFERENCES albums(id) DEFERRABLE INITIALLY DEFERRED,
    kind TEXT NOT NULL CHECK (kind IN ('Album', 'Single', 'Ep', 'Compilation', 'Soundtrack', 'Live', 'Remix', 'Other')),
    soundtrack_source TEXT,
    release_date TEXT CHECK (release_date GLOB '[0-9][0-9][0-9][0-9]' OR release_date GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]' OR release_date GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]'),
    label TEXT,
    copyright TEXT,
    description TEXT,
    visuals_id INTEGER REFERENCES visuals(id) DEFERRABLE INITIALLY DEFERRED,
    CHECK (edition_name IS NOT NULL OR edition_of IS NULL),
    CHECK (kind = 'Soundtrack' OR soundtrack_source IS NULL)
) STRICT;
CREATE INDEX albums_title ON albums (trim(title) COLLATE NOCASE);

CREATE TABLE tracks (
    id TEXT PRIMARY KEY CHECK (
        length(id) = 36 AND substr(id, 9, 1) = '-' AND substr(id, 14, 1) = '-'
        AND substr(id, 19, 1) = '-' AND substr(id, 24, 1) = '-'
        AND length(replace(id, '-', '')) = 32 AND id NOT GLOB '*[^0-9a-f-]*'
    ),
    title TEXT NOT NULL,
    duration_ms INTEGER NOT NULL CHECK (duration_ms BETWEEN 0 AND 4294967295),
    version TEXT,
    album_id TEXT REFERENCES albums(id) DEFERRABLE INITIALLY DEFERRED,
    rating TEXT CHECK (rating IN ('Explicit', 'PartiallyExplicit', 'Clean')),
    about TEXT,
    music_video TEXT,
    visuals_id INTEGER REFERENCES visuals(id) DEFERRABLE INITIALLY DEFERRED,
    tempo_bpm REAL,
    key TEXT CHECK (key IN ('C', 'CSharp', 'D', 'DSharp', 'E', 'F', 'FSharp', 'G', 'GSharp', 'A', 'ASharp', 'B')),
    mode TEXT CHECK (mode IN ('Major', 'Minor')),
    time_signature INTEGER CHECK (time_signature BETWEEN 0 AND 255),
    loudness_lufs REAL,
    peak REAL,
    acousticness INTEGER CHECK (acousticness BETWEEN 0 AND 100),
    danceability INTEGER CHECK (danceability BETWEEN 0 AND 100),
    energy INTEGER CHECK (energy BETWEEN 0 AND 100),
    instrumentalness INTEGER CHECK (instrumentalness BETWEEN 0 AND 100),
    liveness INTEGER CHECK (liveness BETWEEN 0 AND 100),
    speechiness INTEGER CHECK (speechiness BETWEEN 0 AND 100),
    valence INTEGER CHECK (valence BETWEEN 0 AND 100)
) STRICT;
CREATE INDEX tracks_title ON tracks (trim(title) COLLATE NOCASE);
CREATE INDEX tracks_album ON tracks (album_id);


CREATE TABLE tags (
    id TEXT PRIMARY KEY CHECK (
        length(id) = 36 AND substr(id, 9, 1) = '-' AND substr(id, 14, 1) = '-'
        AND substr(id, 19, 1) = '-' AND substr(id, 24, 1) = '-'
        AND length(replace(id, '-', '')) = 32 AND id NOT GLOB '*[^0-9a-f-]*'
    ),
    name TEXT NOT NULL,
    kind TEXT NOT NULL CHECK (kind IN ('Genre', 'Emotion', 'Activity')),
    parent TEXT REFERENCES tags(id) DEFERRABLE INITIALLY DEFERRED
) STRICT;
CREATE INDEX tags_name ON tags (trim(name) COLLATE NOCASE);
