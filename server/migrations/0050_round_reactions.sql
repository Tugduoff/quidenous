-- Live reactions on results (docs/REACTIONS.md): taps from a fixed palette (server/src/reactions.rs) on a
-- revealed round, daily or party. One row per round, member and emoji, `count` the taps (30 per member and
-- round at most, all emojis together). They go with the member's seat (cascade) and their account.
CREATE TABLE round_reactions (
    round_id INTEGER NOT NULL REFERENCES rounds(id),
    member_id INTEGER NOT NULL REFERENCES members(id) ON DELETE CASCADE,
    emoji TEXT NOT NULL,
    count INTEGER NOT NULL,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    PRIMARY KEY (round_id, member_id, emoji)
) WITHOUT ROWID;
CREATE INDEX round_reactions_member ON round_reactions(member_id);

-- The party's toggled reactions become one tap each. `party_reactions` stays (empty) so a rollback still starts.
INSERT INTO round_reactions (round_id, member_id, emoji, count, created_at, updated_at)
SELECT round_id, member_id, emoji, 1, created_at, created_at FROM party_reactions;
DELETE FROM party_reactions;
