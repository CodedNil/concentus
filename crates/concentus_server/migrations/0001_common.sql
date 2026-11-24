-- Shared identity for artists, albums and tracks
CREATE TABLE records (
    id TEXT PRIMARY KEY,
    collection TEXT NOT NULL CHECK (collection IN ('artists', 'albums', 'tracks')),
    last_accessed_at INTEGER,
    UNIQUE (id, collection)
) STRICT;

CREATE TABLE record_access_daily (
    record_id TEXT NOT NULL REFERENCES records(id) ON DELETE CASCADE,
    day INTEGER NOT NULL,
    count INTEGER NOT NULL,
    PRIMARY KEY (record_id, day)
) STRICT, WITHOUT ROWID;
CREATE INDEX record_access_day ON record_access_daily (day, record_id);

-- Tags
CREATE TABLE tags (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    kind TEXT NOT NULL,
    related_id TEXT REFERENCES tags(id) DEFERRABLE INITIALLY DEFERRED,
    relation_kind TEXT CHECK (relation_kind IN ('Parent', 'AliasOf'))
) STRICT;
CREATE INDEX tags_name ON tags (trim(name) COLLATE NOCASE);
CREATE INDEX tags_related ON tags (related_id, relation_kind);

-- Shared metadata
CREATE TABLE artwork (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    owner_id TEXT NOT NULL REFERENCES records(id) ON DELETE CASCADE DEFERRABLE INITIALLY DEFERRED,
    kind TEXT NOT NULL CHECK (kind IN ('Square', 'Banner', 'Portrait', 'MotionBanner', 'MotionSquare', 'Canvas')),
    blurhash TEXT,
    explicit INTEGER NOT NULL CHECK (explicit IN (0, 1)),
    UNIQUE (owner_id, kind)
) STRICT;

CREATE TRIGGER artwork_slot_insert BEFORE INSERT ON artwork
WHEN NOT EXISTS (
    SELECT 1 FROM records
    WHERE id = new.owner_id AND (
        (collection = 'artists' AND new.kind IN ('Square', 'Banner', 'Portrait', 'MotionBanner'))
        OR (collection = 'albums' AND new.kind IN ('Square', 'Banner', 'MotionSquare'))
        OR (collection = 'tracks' AND new.kind = 'Canvas')
    )
)
BEGIN
    SELECT RAISE(ABORT, 'Artwork slot does not belong to this collection');
END;
CREATE TRIGGER artwork_slot_update BEFORE UPDATE OF owner_id, kind ON artwork
WHEN NOT EXISTS (
    SELECT 1 FROM records
    WHERE id = new.owner_id AND (
        (collection = 'artists' AND new.kind IN ('Square', 'Banner', 'Portrait', 'MotionBanner'))
        OR (collection = 'albums' AND new.kind IN ('Square', 'Banner', 'MotionSquare'))
        OR (collection = 'tracks' AND new.kind = 'Canvas')
    )
)
BEGIN
    SELECT RAISE(ABORT, 'Artwork slot does not belong to this collection');
END;

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
    last_error TEXT,
    PRIMARY KEY (record_id, field)
) STRICT, WITHOUT ROWID;
CREATE INDEX enrichment_due ON enrichment_needed (due_at);

CREATE TABLE record_history (
    record_id TEXT NOT NULL REFERENCES records(id) ON DELETE CASCADE,
    field TEXT NOT NULL,
    minute INTEGER NOT NULL,
    old_value TEXT NOT NULL,
    new_value TEXT NOT NULL,
    source TEXT NOT NULL DEFAULT 'Editor',
    checked_at INTEGER,
    check_source TEXT,
    outcome TEXT,
    PRIMARY KEY (record_id, field, minute)
) STRICT, WITHOUT ROWID;

-- Catalog full-text search
CREATE VIRTUAL TABLE catalog_search USING fts5(
    record_id UNINDEXED, collection UNINDEXED, title,
    tokenize = 'unicode61 remove_diacritics 2', prefix = '2 3 4'
);

CREATE TRIGGER search_tags_insert AFTER INSERT ON tags BEGIN
    INSERT INTO catalog_search (record_id, collection, title) VALUES (new.id, 'tags', new.name);
END;
CREATE TRIGGER search_tags_update AFTER UPDATE OF id, name ON tags
WHEN old.id IS NOT new.id OR old.name IS NOT new.name BEGIN
    DELETE FROM catalog_search WHERE record_id = old.id AND collection = 'tags';
    INSERT INTO catalog_search (record_id, collection, title) VALUES (new.id, 'tags', new.name);
END;
CREATE TRIGGER search_tags_delete AFTER DELETE ON tags BEGIN
    DELETE FROM catalog_search WHERE record_id = old.id AND collection = 'tags';
END;
