-- "Let's not wait for them": per open question, who agreed to skip whom.
CREATE TABLE skips (
    round_id INTEGER NOT NULL REFERENCES rounds(id),
    from_id INTEGER NOT NULL REFERENCES members(id),
    target_id INTEGER NOT NULL REFERENCES members(id),
    PRIMARY KEY (round_id, from_id, target_id)
);

-- Votes to remove someone from the group; removed members keep their past votes in the history.
CREATE TABLE kicks (
    target_id INTEGER NOT NULL REFERENCES members(id),
    from_id INTEGER NOT NULL REFERENCES members(id),
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    PRIMARY KEY (target_id, from_id)
);
ALTER TABLE members ADD COLUMN removed_at TEXT;
