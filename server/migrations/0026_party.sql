-- Party mode (docs/PARTY.md): a room with `kind = 'party'`, one question at a time for everyone in
-- the same place, a countdown per question, "Suivante" taps, quick reactions, guests without an account.

ALTER TABLE groups ADD COLUMN kind TEXT NOT NULL DEFAULT 'room' CHECK (kind IN ('room', 'party'));
-- After it the party is over: a read-only memory.
ALTER TABLE groups ADD COLUMN party_ends_at TEXT;
-- Seconds per question (15, 20, 30, 45, 60).
ALTER TABLE groups ADD COLUMN party_timer INTEGER NOT NULL DEFAULT 30;
-- When the server processed the end (open question closed, room paused, everyone told). Internal.
ALTER TABLE groups ADD COLUMN party_ended_at TEXT;
CREATE INDEX groups_party_ends ON groups(party_ends_at)
    WHERE kind = 'party' AND party_ended_at IS NULL;

-- Party questions close at their deadline (opened_at + timer), voted or not.
ALTER TABLE rounds ADD COLUMN deadline_at TEXT;
CREATE INDEX rounds_open_deadline ON rounds(deadline_at)
    WHERE closed_at IS NULL AND deadline_at IS NOT NULL;

-- Who was in the party when the question opened: only they vote on it and can be picked.
CREATE TABLE party_players (
    round_id INTEGER NOT NULL REFERENCES rounds(id),
    member_id INTEGER NOT NULL REFERENCES members(id),
    PRIMARY KEY (round_id, member_id)
) WITHOUT ROWID;

-- "Suivante" taps on a revealed question.
CREATE TABLE party_taps (
    round_id INTEGER NOT NULL REFERENCES rounds(id),
    member_id INTEGER NOT NULL REFERENCES members(id),
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    PRIMARY KEY (round_id, member_id)
) WITHOUT ROWID;

-- Quick reactions on a result, toggled.
CREATE TABLE party_reactions (
    round_id INTEGER NOT NULL REFERENCES rounds(id),
    member_id INTEGER NOT NULL REFERENCES members(id),
    emoji TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    PRIMARY KEY (round_id, member_id, emoji)
) WITHOUT ROWID;

-- Accounts made by joining a party without one (token only). `guest` reads false once secured.
ALTER TABLE users ADD COLUMN guest INTEGER NOT NULL DEFAULT 0;
