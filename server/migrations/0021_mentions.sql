-- @mentions in the chat: who a message pings (member ids of the room, JSON array) and @tous.
ALTER TABLE messages ADD COLUMN mentions TEXT NOT NULL DEFAULT '[]';
ALTER TABLE messages ADD COLUMN everyone INTEGER NOT NULL DEFAULT 0;

-- Inbox gets a new kind ("X mentioned you"): SQLite can't change a CHECK, so rebuild the table.
CREATE TABLE inbox_new (
    id INTEGER PRIMARY KEY,
    member_id INTEGER NOT NULL REFERENCES members(id),
    kind TEXT NOT NULL CHECK (kind IN ('message', 'nudge', 'result', 'question', 'added', 'ask', 'mention')),
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
