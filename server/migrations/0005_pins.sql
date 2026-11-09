-- Results pinned to the top of the history, for the whole group.
CREATE TABLE pins (
    round_id INTEGER PRIMARY KEY REFERENCES rounds(id),
    member_id INTEGER NOT NULL REFERENCES members(id),
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);
