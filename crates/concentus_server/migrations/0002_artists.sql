-- Numeric provider IDs use checked decimal text to preserve the full unsigned 64-bit range.
CREATE TABLE artist_members (
    id INTEGER PRIMARY KEY,
    owner_id TEXT NOT NULL REFERENCES artists(id) ON DELETE CASCADE DEFERRABLE INITIALLY DEFERRED,
    position INTEGER NOT NULL CHECK (position >= 0),
    artist_id TEXT NOT NULL REFERENCES artists(id) DEFERRABLE INITIALLY DEFERRED,
    UNIQUE (owner_id, position)
) STRICT;

CREATE TABLE member_roles (
    owner_id INTEGER NOT NULL REFERENCES artist_members(id) ON DELETE CASCADE DEFERRABLE INITIALLY DEFERRED,
    position INTEGER NOT NULL CHECK (position >= 0),
    role TEXT NOT NULL CHECK (role IN ('Primary', 'Performer', 'Vocals', 'BackgroundVocals', 'Instrument', 'Featured', 'Composer', 'Lyricist', 'Writer', 'Arranger', 'Producer', 'ExecutiveProducer', 'Engineer', 'Mixer', 'Mastering', 'Remixer', 'Other')),
    role_detail TEXT,
    PRIMARY KEY (owner_id, position),
    CHECK ((role IN ('Instrument', 'Other')) = (role_detail IS NOT NULL))
) STRICT, WITHOUT ROWID;

CREATE TABLE artist_tags (
    owner_id TEXT NOT NULL REFERENCES artists(id) ON DELETE CASCADE DEFERRABLE INITIALLY DEFERRED,
    position INTEGER NOT NULL CHECK (position >= 0),
    tag_id TEXT NOT NULL REFERENCES tags(id) DEFERRABLE INITIALLY DEFERRED,
    weight INTEGER NOT NULL CHECK (weight BETWEEN 0 AND 100),
    PRIMARY KEY (owner_id, position)
) STRICT, WITHOUT ROWID;

CREATE TABLE artist_external_ids (
    owner_id TEXT NOT NULL REFERENCES artists(id) ON DELETE CASCADE DEFERRABLE INITIALLY DEFERRED,
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
CREATE INDEX artist_external_ids_provider ON artist_external_ids (provider, value_text, value_number, other_name, owner_id);
