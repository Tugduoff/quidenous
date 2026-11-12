-- Public share links: an unguessable token shows one question (and its podium if chosen), nothing else.
CREATE TABLE shares (
    token TEXT PRIMARY KEY,
    round_id INTEGER NOT NULL REFERENCES rounds(id),
    member_id INTEGER NOT NULL REFERENCES members(id),
    with_result INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);
