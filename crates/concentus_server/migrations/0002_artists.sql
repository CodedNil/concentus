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
    theme_font_url TEXT,
    record_collection TEXT GENERATED ALWAYS AS ('artists') VIRTUAL,
    FOREIGN KEY (id, record_collection) REFERENCES records(id, collection) ON DELETE CASCADE DEFERRABLE INITIALLY DEFERRED
) STRICT;
CREATE INDEX artists_name ON artists (trim(name) COLLATE NOCASE);

CREATE TABLE artist_members (
    id INTEGER PRIMARY KEY,
    owner_id TEXT NOT NULL REFERENCES artists(id) ON DELETE CASCADE DEFERRABLE INITIALLY DEFERRED,
    position INTEGER NOT NULL,
    artist_id TEXT NOT NULL REFERENCES artists(id) DEFERRABLE INITIALLY DEFERRED,
    joined_date TEXT,
    left_date TEXT,
    UNIQUE (owner_id, position)
) STRICT;
CREATE INDEX artist_members_artist ON artist_members (artist_id, owner_id);

CREATE TABLE member_roles (
    owner_id INTEGER NOT NULL REFERENCES artist_members(id) ON DELETE CASCADE DEFERRABLE INITIALLY DEFERRED,
    position INTEGER NOT NULL,
    role TEXT NOT NULL,
    role_detail TEXT,
    PRIMARY KEY (owner_id, position)
) STRICT, WITHOUT ROWID;
