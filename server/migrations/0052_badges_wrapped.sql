-- Titles & badges (docs/BADGES.md) and the Wrapped (docs/WRAPPED.md).

-- Badges earned, one row per tier reached (1 bronze, 2 silver, 3 gold). Never taken back.
-- `group_id`: the room where the threshold was crossed (null when unknown: hourly sweep, backfill).
CREATE TABLE user_badges (
    user_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    key TEXT NOT NULL,
    tier INTEGER NOT NULL CHECK (tier BETWEEN 1 AND 3),
    group_id INTEGER REFERENCES groups(id) ON DELETE SET NULL,
    earned_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    PRIMARY KEY (user_id, key, tier)
);

-- The badge key shown next to the name (null = none); the last evaluation (null = never: the
-- first one backfills silently, no inbox flood on deploy); the year the "your Wrapped is ready"
-- inbox entry went out.
ALTER TABLE users ADD COLUMN title TEXT;
ALTER TABLE users ADD COLUMN badges_at TEXT;
ALTER TABLE users ADD COLUMN wrapped_notified INTEGER;

-- Wrapped share cards show this in the frame instead of the room name.
ALTER TABLE stat_shares ADD COLUMN label TEXT;

-- The per-account metrics read a person's votes, nudges and bets across their seats: without
-- these, each evaluation scanned the whole table.
CREATE INDEX IF NOT EXISTS votes_voter ON votes(voter_id);
CREATE INDEX IF NOT EXISTS nudges_from ON nudges(from_id);
CREATE INDEX IF NOT EXISTS tripot_bets_member ON tripot_bets(member_id);

-- Inbox gets two kinds ("New title: …", "Your year is ready"): SQLite can't change a CHECK, so
-- rebuild the table (as in 0038), with its indexes (0038, 0043).
CREATE TABLE inbox_new (
    id INTEGER PRIMARY KEY,
    member_id INTEGER NOT NULL REFERENCES members(id),
    kind TEXT NOT NULL CHECK (kind IN ('message', 'nudge', 'result', 'question', 'added', 'ask', 'mention',
                                       'friend_request', 'invite', 'owner', 'badge', 'wrapped')),
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
CREATE INDEX inbox_unread ON inbox(member_id) WHERE read_at IS NULL;
CREATE INDEX inbox_actor ON inbox(actor_id);
