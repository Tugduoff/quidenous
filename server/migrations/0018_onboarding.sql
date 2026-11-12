-- Vibes: built-in deck questions carry tags (`potes,famille`), a room picks the vibe it draws from.
ALTER TABLE questions ADD COLUMN vibes TEXT;
ALTER TABLE groups ADD COLUMN vibe TEXT;

-- Inbox gets a new kind ("X asks for questions", to the owner): SQLite can't change a CHECK, so rebuild the table.
CREATE TABLE inbox_new (
    id INTEGER PRIMARY KEY,
    member_id INTEGER NOT NULL REFERENCES members(id),
    kind TEXT NOT NULL CHECK (kind IN ('message', 'nudge', 'result', 'question', 'added', 'ask')),
    round_id INTEGER REFERENCES rounds(id),
    actor_id INTEGER REFERENCES members(id),
    text TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    read_at TEXT
);
INSERT INTO inbox_new SELECT id, member_id, kind, round_id, actor_id, text, created_at, read_at FROM inbox;
DROP TABLE inbox;
ALTER TABLE inbox_new RENAME TO inbox;
CREATE INDEX inbox_member ON inbox(member_id, id);
