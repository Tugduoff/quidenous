-- Moderation (docs/AUTH.md, "Reports"): chat messages hidden by reports, reports an admin closes,
-- suggestion reports, suspended accounts and locked parties.

-- Set when three different people reported the message: nobody sees it until an admin decides
-- (dismiss = visible again, delete = gone).
ALTER TABLE messages ADD COLUMN hidden_at TEXT;

-- Reports get a fourth kind (a suggestion to the shared deck: no room) and a resolution. SQLite
-- can't change a CHECK, so rebuild the table (as in 0021).
CREATE TABLE reports_new (
    id INTEGER PRIMARY KEY,
    reporter_id INTEGER NOT NULL REFERENCES users(id),
    kind TEXT NOT NULL CHECK (kind IN ('message', 'member', 'question', 'suggestion')),
    target_id INTEGER NOT NULL,
    room_id INTEGER REFERENCES groups(id),
    reported_user_id INTEGER REFERENCES users(id),
    content TEXT NOT NULL,
    reason TEXT NOT NULL CHECK (reason IN ('offensive', 'harassment', 'sexual', 'spam', 'other')),
    details TEXT,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    -- When an admin closed it, and how: dismiss, delete, suspend, erase.
    resolved_at TEXT,
    resolution TEXT
);
INSERT INTO reports_new (id, reporter_id, kind, target_id, room_id, reported_user_id, content, reason, details, created_at)
    SELECT id, reporter_id, kind, target_id, room_id, reported_user_id, content, reason, details, created_at FROM reports;
DROP TABLE reports;
ALTER TABLE reports_new RENAME TO reports;
CREATE UNIQUE INDEX reports_once ON reports(reporter_id, kind, target_id);
CREATE INDEX reports_open ON reports(kind, target_id) WHERE resolved_at IS NULL;

-- An admin suspended the account: its sessions are gone and it can't sign in again.
ALTER TABLE users ADD COLUMN suspended_at TEXT;

-- An admin approved the pack after reports: editing it again sends it back to review.
ALTER TABLE packs ADD COLUMN approved_at TEXT;

-- The party owner closed the door: nobody new can join (seats already taken stay).
ALTER TABLE groups ADD COLUMN party_locked INTEGER NOT NULL DEFAULT 0;
