-- Shared stat stories: a frozen snapshot (title, figure, punchline, first names and colours of the
-- people in it), so the public card never shows anything else and doesn't change afterwards.
CREATE TABLE stat_shares (
    token TEXT PRIMARY KEY,
    group_id INTEGER NOT NULL REFERENCES groups(id),
    member_id INTEGER NOT NULL REFERENCES members(id),
    key TEXT NOT NULL,
    title TEXT NOT NULL,
    big TEXT NOT NULL,
    line TEXT NOT NULL,
    -- JSON [[first name, colour], ...]
    people TEXT NOT NULL DEFAULT '[]',
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);
