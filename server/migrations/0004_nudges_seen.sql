-- Last round whose result the member has seen in the app (round number).
ALTER TABLE members ADD COLUMN seen_round INTEGER NOT NULL DEFAULT 0;
UPDATE members SET seen_round = COALESCE(
    (SELECT MAX(number) FROM rounds WHERE rounds.group_id = members.group_id AND closed_at IS NOT NULL), 0);

-- One nudge per person and per question.
CREATE TABLE nudges (
    round_id INTEGER NOT NULL REFERENCES rounds(id),
    from_id INTEGER NOT NULL REFERENCES members(id),
    to_id INTEGER NOT NULL REFERENCES members(id),
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    PRIMARY KEY (round_id, from_id, to_id)
);
