CREATE TABLE visuals (
    id INTEGER PRIMARY KEY,
    light_scheme_id INTEGER REFERENCES schemes(id) DEFERRABLE INITIALLY DEFERRED,
    dark_scheme_id INTEGER REFERENCES schemes(id) DEFERRABLE INITIALLY DEFERRED,
    font_id INTEGER REFERENCES fonts(id) DEFERRABLE INITIALLY DEFERRED
) STRICT;

CREATE TABLE schemes (
    id INTEGER PRIMARY KEY,
    background TEXT NOT NULL CHECK (background GLOB '#[0-9a-fA-F][0-9a-fA-F][0-9a-fA-F][0-9a-fA-F][0-9a-fA-F][0-9a-fA-F]'),
    foreground TEXT NOT NULL CHECK (foreground GLOB '#[0-9a-fA-F][0-9a-fA-F][0-9a-fA-F][0-9a-fA-F][0-9a-fA-F][0-9a-fA-F]'),
    accent TEXT NOT NULL CHECK (accent GLOB '#[0-9a-fA-F][0-9a-fA-F][0-9a-fA-F][0-9a-fA-F][0-9a-fA-F][0-9a-fA-F]')
) STRICT;

CREATE TABLE fonts (
    id INTEGER PRIMARY KEY,
    family TEXT NOT NULL,
    url TEXT NOT NULL
) STRICT;

CREATE TABLE artwork (
    id INTEGER PRIMARY KEY,
    owner_id INTEGER NOT NULL REFERENCES visuals(id) ON DELETE CASCADE DEFERRABLE INITIALLY DEFERRED,
    position INTEGER NOT NULL CHECK (position >= 0),
    kind TEXT NOT NULL CHECK (kind IN ('Cover', 'Back', 'Portrait', 'Banner', 'Logo', 'Canvas')),
    blurhash TEXT,
    explicit INTEGER NOT NULL CHECK (explicit BETWEEN 0 AND 1),
    UNIQUE (owner_id, position)
) STRICT;

CREATE TABLE artwork_renditions (
    owner_id INTEGER NOT NULL REFERENCES artwork(id) ON DELETE CASCADE DEFERRABLE INITIALLY DEFERRED,
    position INTEGER NOT NULL CHECK (position >= 0),
    url TEXT NOT NULL,
    width INTEGER NOT NULL CHECK (width BETWEEN 0 AND 65535),
    height INTEGER NOT NULL CHECK (height BETWEEN 0 AND 65535),
    codec TEXT,
    duration_ms INTEGER CHECK (duration_ms BETWEEN 0 AND 4294967295),
    PRIMARY KEY (owner_id, position)
) STRICT, WITHOUT ROWID;

CREATE TABLE artwork_colors (
    owner_id INTEGER NOT NULL REFERENCES artwork(id) ON DELETE CASCADE DEFERRABLE INITIALLY DEFERRED,
    position INTEGER NOT NULL CHECK (position >= 0),
    color TEXT NOT NULL CHECK (color GLOB '#[0-9a-fA-F][0-9a-fA-F][0-9a-fA-F][0-9a-fA-F][0-9a-fA-F][0-9a-fA-F]'),
    PRIMARY KEY (owner_id, position)
) STRICT, WITHOUT ROWID;
