-- Two kinds of question: pick one person ('pick'), or rank everyone from first to last ('rank').
ALTER TABLE questions ADD COLUMN kind TEXT NOT NULL DEFAULT 'pick' CHECK (kind IN ('pick', 'rank'));

-- A ranking vote: each voter puts every member at a position (1 = first). The votes row keeps their #1.
CREATE TABLE rankings (
    round_id INTEGER NOT NULL REFERENCES rounds(id),
    voter_id INTEGER NOT NULL REFERENCES members(id),
    member_id INTEGER NOT NULL REFERENCES members(id),
    position INTEGER NOT NULL,
    PRIMARY KEY (round_id, voter_id, member_id)
);
