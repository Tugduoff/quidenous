-- Community question packs: published by anyone, installed into a room's queue in one tap.
CREATE TABLE packs (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    author_id INTEGER NOT NULL REFERENCES users(id),
    installs INTEGER NOT NULL DEFAULT 0,
    hidden INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);
CREATE TABLE pack_questions (
    pack_id INTEGER NOT NULL REFERENCES packs(id),
    position INTEGER NOT NULL,
    text TEXT NOT NULL,
    kind TEXT NOT NULL DEFAULT 'pick',
    PRIMARY KEY (pack_id, position)
);
-- One report per person; enough reports hide a pack.
CREATE TABLE pack_reports (
    pack_id INTEGER NOT NULL REFERENCES packs(id),
    user_id INTEGER NOT NULL REFERENCES users(id),
    PRIMARY KEY (pack_id, user_id)
);
