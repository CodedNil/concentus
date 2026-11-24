-- Shared identity for artists, albums and tracks
CREATE TABLE records (
    id TEXT PRIMARY KEY,
    collection TEXT NOT NULL CHECK (collection IN ('artists', 'albums', 'tracks')),
    UNIQUE (id, collection)
) STRICT;

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

-- Shared metadata
CREATE TABLE artwork (
    id INTEGER PRIMARY KEY,
    owner_id TEXT NOT NULL REFERENCES records(id) ON DELETE CASCADE DEFERRABLE INITIALLY DEFERRED,
    position INTEGER NOT NULL,
    kind TEXT NOT NULL,
    blurhash TEXT,
    explicit INTEGER NOT NULL CHECK (explicit IN (0, 1)),
    UNIQUE (owner_id, position)
) STRICT;

CREATE TABLE artwork_renditions (
    owner_id INTEGER NOT NULL REFERENCES artwork(id) ON DELETE CASCADE DEFERRABLE INITIALLY DEFERRED,
    position INTEGER NOT NULL,
    url TEXT NOT NULL,
    width INTEGER NOT NULL,
    height INTEGER NOT NULL,
    codec TEXT,
    duration_ms INTEGER,
    PRIMARY KEY (owner_id, position)
) STRICT, WITHOUT ROWID;

CREATE TABLE artwork_colors (
    owner_id INTEGER NOT NULL REFERENCES artwork(id) ON DELETE CASCADE DEFERRABLE INITIALLY DEFERRED,
    position INTEGER NOT NULL,
    color TEXT NOT NULL,
    PRIMARY KEY (owner_id, position)
) STRICT, WITHOUT ROWID;

CREATE TABLE record_tags (
    owner_id TEXT NOT NULL REFERENCES records(id) ON DELETE CASCADE DEFERRABLE INITIALLY DEFERRED,
    tag_id TEXT NOT NULL REFERENCES tags(id) DEFERRABLE INITIALLY DEFERRED,
    weight INTEGER NOT NULL CHECK (weight BETWEEN 0 AND 100),
    PRIMARY KEY (owner_id, tag_id)
) STRICT, WITHOUT ROWID;
CREATE INDEX record_tags_tag ON record_tags (tag_id, owner_id);

CREATE TABLE external_ids (
    owner_id TEXT NOT NULL REFERENCES records(id) ON DELETE CASCADE DEFERRABLE INITIALLY DEFERRED,
    provider TEXT NOT NULL,
    value TEXT NOT NULL,
    PRIMARY KEY (owner_id, provider, value)
) STRICT, WITHOUT ROWID;

CREATE TABLE credits (
    owner_id TEXT NOT NULL REFERENCES records(id) ON DELETE CASCADE DEFERRABLE INITIALLY DEFERRED,
    position INTEGER NOT NULL,
    artist_id TEXT NOT NULL REFERENCES artists(id) DEFERRABLE INITIALLY DEFERRED,
    role TEXT NOT NULL,
    role_detail TEXT,
    PRIMARY KEY (owner_id, position)
) STRICT, WITHOUT ROWID;
CREATE INDEX credits_artist ON credits (artist_id, role, owner_id);
CREATE INDEX external_ids_provider ON external_ids (provider, value, owner_id);

-- Enrichment queue and revision history
CREATE TABLE enrichment_needed (
    record_id TEXT NOT NULL REFERENCES records(id) ON DELETE CASCADE,
    field TEXT NOT NULL,
    due_at INTEGER NOT NULL,
    generation INTEGER NOT NULL,
    refresh INTEGER NOT NULL CHECK (refresh IN (0, 1)),
    PRIMARY KEY (record_id, field)
) STRICT, WITHOUT ROWID;
CREATE INDEX enrichment_due ON enrichment_needed (due_at);

CREATE TABLE record_history (
    record_id TEXT NOT NULL REFERENCES records(id) ON DELETE CASCADE,
    field TEXT NOT NULL,
    minute INTEGER NOT NULL,
    old_value TEXT NOT NULL,
    new_value TEXT NOT NULL,
    PRIMARY KEY (record_id, field, minute)
) STRICT, WITHOUT ROWID;
