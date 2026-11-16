-- Privacy & Play compliance (docs/AUTH.md "Terms", docs/STATS.md "Share links").

-- The terms version a user accepted (16+ included) and when; null = never: the app asks once.
ALTER TABLE users ADD COLUMN terms_version INTEGER;
ALTER TABLE users ADD COLUMN terms_at TEXT;

-- Share links expire (new ones after 90 days; null = made before expiries, never expires) and can be
-- revoked by their sharer or the room's owner. Stat cards remember who is on them, so erasing an
-- account can remove the cards that show it (null = older card: matched by first name instead).
ALTER TABLE shares ADD COLUMN expires_at TEXT;
ALTER TABLE stat_shares ADD COLUMN expires_at TEXT;
ALTER TABLE stat_shares ADD COLUMN member_ids TEXT;
CREATE INDEX IF NOT EXISTS shares_member ON shares(member_id);
CREATE INDEX IF NOT EXISTS stat_shares_member ON stat_shares(member_id);

-- A challenge shows the source room's podium to the other room only when it came from a share that
-- showed the result, or between two rooms of the same person. Older challenges: the latter only.
ALTER TABLE challenges ADD COLUMN with_result INTEGER NOT NULL DEFAULT 1;
UPDATE challenges SET with_result = EXISTS (
    SELECT 1 FROM rounds r JOIN members m ON m.group_id = r.group_id
    WHERE r.id = challenges.from_round_id AND m.user_id = challenges.created_by
);
