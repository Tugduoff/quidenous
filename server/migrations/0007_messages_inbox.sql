-- Chat under a finished question's result.
CREATE TABLE messages (
    id INTEGER PRIMARY KEY,
    round_id INTEGER NOT NULL REFERENCES rounds(id),
    member_id INTEGER NOT NULL REFERENCES members(id),
    text TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);
CREATE INDEX messages_round ON messages(round_id, id);

-- In-app notifications (the inbox): one row per recipient.
CREATE TABLE inbox (
    id INTEGER PRIMARY KEY,
    member_id INTEGER NOT NULL REFERENCES members(id),
    kind TEXT NOT NULL CHECK (kind IN ('message', 'nudge', 'result', 'question')),
    round_id INTEGER REFERENCES rounds(id),
    actor_id INTEGER REFERENCES members(id),
    text TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    read_at TEXT
);
CREATE INDEX inbox_member ON inbox(member_id, id);
