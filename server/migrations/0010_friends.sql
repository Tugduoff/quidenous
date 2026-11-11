-- Friends added by code (both directions stored). People who share a room count as friends too, computed.
CREATE TABLE friends (
    user_id INTEGER NOT NULL REFERENCES users(id),
    friend_id INTEGER NOT NULL REFERENCES users(id),
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    PRIMARY KEY (user_id, friend_id)
);

-- Inbox gets a new kind ("X added you to a room"): SQLite can't change a CHECK, so rebuild the table.
CREATE TABLE inbox_new (
    id INTEGER PRIMARY KEY,
    member_id INTEGER NOT NULL REFERENCES members(id),
    kind TEXT NOT NULL CHECK (kind IN ('message', 'nudge', 'result', 'question', 'added')),
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
