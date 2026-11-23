-- Numeric provider IDs use checked decimal text to preserve the full unsigned 64-bit range.
CREATE TABLE album_tags (
    owner_id TEXT NOT NULL REFERENCES albums(id) ON DELETE CASCADE DEFERRABLE INITIALLY DEFERRED,
    position INTEGER NOT NULL CHECK (position >= 0),
    tag_id TEXT NOT NULL REFERENCES tags(id) DEFERRABLE INITIALLY DEFERRED,
    weight INTEGER NOT NULL CHECK (weight BETWEEN 0 AND 100),
    PRIMARY KEY (owner_id, position)
) STRICT, WITHOUT ROWID;

CREATE TABLE track_tags (
    owner_id TEXT NOT NULL REFERENCES tracks(id) ON DELETE CASCADE DEFERRABLE INITIALLY DEFERRED,
    position INTEGER NOT NULL CHECK (position >= 0),
    tag_id TEXT NOT NULL REFERENCES tags(id) DEFERRABLE INITIALLY DEFERRED,
    weight INTEGER NOT NULL CHECK (weight BETWEEN 0 AND 100),
    PRIMARY KEY (owner_id, position)
) STRICT, WITHOUT ROWID;

CREATE TABLE album_external_ids (
    owner_id TEXT NOT NULL REFERENCES albums(id) ON DELETE CASCADE DEFERRABLE INITIALLY DEFERRED,
    position INTEGER NOT NULL CHECK (position >= 0),
    provider TEXT NOT NULL CHECK (provider IN ('Musicbrainz', 'Isrc', 'Barcode', 'Wikidata', 'Discogs', 'Genius', 'AppleMusic', 'Deezer', 'Tidal', 'Spotify', 'Amazon', 'YoutubeMusic', 'Isni', 'Ipi', 'Other')),
    value_text TEXT,
    value_number TEXT CHECK (length(value_number) BETWEEN 1 AND 20 AND value_number NOT GLOB '*[^0-9]*'
        AND (value_number = '0' OR substr(value_number, 1, 1) <> '0')
        AND (length(value_number) < 20 OR value_number <= '18446744073709551615')),
    other_name TEXT,
    PRIMARY KEY (owner_id, position),
    CHECK (
        (provider IN ('Barcode', 'Wikidata', 'Discogs', 'Genius', 'AppleMusic', 'Deezer', 'Tidal', 'Ipi')
            AND value_number IS NOT NULL AND value_text IS NULL AND other_name IS NULL)
        OR (provider IN ('Musicbrainz', 'Isrc', 'Spotify', 'Amazon', 'YoutubeMusic', 'Isni')
            AND value_number IS NULL AND value_text IS NOT NULL AND other_name IS NULL)
        OR (provider = 'Other' AND value_number IS NULL AND value_text IS NOT NULL AND other_name IS NOT NULL)
    )
) STRICT, WITHOUT ROWID;
CREATE INDEX album_external_ids_provider ON album_external_ids (provider, value_text, value_number, other_name, owner_id);

CREATE TABLE track_external_ids (
    owner_id TEXT NOT NULL REFERENCES tracks(id) ON DELETE CASCADE DEFERRABLE INITIALLY DEFERRED,
    position INTEGER NOT NULL CHECK (position >= 0),
    provider TEXT NOT NULL CHECK (provider IN ('Musicbrainz', 'Isrc', 'Barcode', 'Wikidata', 'Discogs', 'Genius', 'AppleMusic', 'Deezer', 'Tidal', 'Spotify', 'Amazon', 'YoutubeMusic', 'Isni', 'Ipi', 'Other')),
    value_text TEXT,
    value_number TEXT CHECK (length(value_number) BETWEEN 1 AND 20 AND value_number NOT GLOB '*[^0-9]*'
        AND (value_number = '0' OR substr(value_number, 1, 1) <> '0')
        AND (length(value_number) < 20 OR value_number <= '18446744073709551615')),
    other_name TEXT,
    PRIMARY KEY (owner_id, position),
    CHECK (
        (provider IN ('Barcode', 'Wikidata', 'Discogs', 'Genius', 'AppleMusic', 'Deezer', 'Tidal', 'Ipi')
            AND value_number IS NOT NULL AND value_text IS NULL AND other_name IS NULL)
        OR (provider IN ('Musicbrainz', 'Isrc', 'Spotify', 'Amazon', 'YoutubeMusic', 'Isni')
            AND value_number IS NULL AND value_text IS NOT NULL AND other_name IS NULL)
        OR (provider = 'Other' AND value_number IS NULL AND value_text IS NOT NULL AND other_name IS NOT NULL)
    )
) STRICT, WITHOUT ROWID;
CREATE INDEX track_external_ids_provider ON track_external_ids (provider, value_text, value_number, other_name, owner_id);

CREATE TABLE album_discs (
    id INTEGER PRIMARY KEY,
    owner_id TEXT NOT NULL REFERENCES albums(id) ON DELETE CASCADE DEFERRABLE INITIALLY DEFERRED,
    position INTEGER NOT NULL CHECK (position >= 0),
    title TEXT,
    UNIQUE (owner_id, position)
) STRICT;

CREATE TABLE disc_tracks (
    owner_id INTEGER NOT NULL REFERENCES album_discs(id) ON DELETE CASCADE DEFERRABLE INITIALLY DEFERRED,
    position INTEGER NOT NULL CHECK (position >= 0),
    track_id TEXT NOT NULL REFERENCES tracks(id) DEFERRABLE INITIALLY DEFERRED,
    PRIMARY KEY (owner_id, position)
) STRICT, WITHOUT ROWID;

CREATE TABLE album_credits (
    owner_id TEXT NOT NULL REFERENCES albums(id) ON DELETE CASCADE DEFERRABLE INITIALLY DEFERRED,
    position INTEGER NOT NULL CHECK (position >= 0),
    artist_id TEXT NOT NULL REFERENCES artists(id) DEFERRABLE INITIALLY DEFERRED,
    role TEXT NOT NULL CHECK (role IN ('Primary', 'Performer', 'Vocals', 'BackgroundVocals', 'Instrument', 'Featured', 'Composer', 'Lyricist', 'Writer', 'Arranger', 'Producer', 'ExecutiveProducer', 'Engineer', 'Mixer', 'Mastering', 'Remixer', 'Other')),
    role_detail TEXT,
    PRIMARY KEY (owner_id, position),
    CHECK ((role IN ('Instrument', 'Other')) = (role_detail IS NOT NULL))
) STRICT, WITHOUT ROWID;
CREATE INDEX album_credits_artist ON album_credits (artist_id, role, owner_id);

CREATE TABLE track_credits (
    owner_id TEXT NOT NULL REFERENCES tracks(id) ON DELETE CASCADE DEFERRABLE INITIALLY DEFERRED,
    position INTEGER NOT NULL CHECK (position >= 0),
    artist_id TEXT NOT NULL REFERENCES artists(id) DEFERRABLE INITIALLY DEFERRED,
    role TEXT NOT NULL CHECK (role IN ('Primary', 'Performer', 'Vocals', 'BackgroundVocals', 'Instrument', 'Featured', 'Composer', 'Lyricist', 'Writer', 'Arranger', 'Producer', 'ExecutiveProducer', 'Engineer', 'Mixer', 'Mastering', 'Remixer', 'Other')),
    role_detail TEXT,
    PRIMARY KEY (owner_id, position),
    CHECK ((role IN ('Instrument', 'Other')) = (role_detail IS NOT NULL))
) STRICT, WITHOUT ROWID;
CREATE INDEX track_credits_artist ON track_credits (artist_id, role, owner_id);

CREATE TABLE track_languages (
    owner_id TEXT NOT NULL REFERENCES tracks(id) ON DELETE CASCADE DEFERRABLE INITIALLY DEFERRED,
    position INTEGER NOT NULL CHECK (position >= 0),
    language TEXT NOT NULL,
    PRIMARY KEY (owner_id, position)
) STRICT, WITHOUT ROWID;

CREATE TABLE track_derivations (
    owner_id TEXT NOT NULL REFERENCES tracks(id) ON DELETE CASCADE DEFERRABLE INITIALLY DEFERRED,
    position INTEGER NOT NULL CHECK (position >= 0),
    track_id TEXT NOT NULL REFERENCES tracks(id) DEFERRABLE INITIALLY DEFERRED,
    kind TEXT NOT NULL CHECK (kind IN ('Sample', 'Interpolation', 'Cover', 'Remix', 'Version')),
    from_start_ms INTEGER CHECK (from_start_ms BETWEEN 0 AND 4294967295),
    from_end_ms INTEGER CHECK (from_end_ms BETWEEN 0 AND 4294967295),
    to_start_ms INTEGER CHECK (to_start_ms BETWEEN 0 AND 4294967295),
    to_end_ms INTEGER CHECK (to_end_ms BETWEEN 0 AND 4294967295),
    PRIMARY KEY (owner_id, position),
    CHECK ((from_start_ms IS NULL) = (from_end_ms IS NULL)),
    CHECK ((to_start_ms IS NULL) = (to_end_ms IS NULL)),
    CHECK (kind = 'Sample' OR (from_start_ms IS NULL AND to_start_ms IS NULL))
) STRICT, WITHOUT ROWID;
