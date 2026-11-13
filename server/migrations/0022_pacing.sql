-- Room pacing (owner settings, off by default = the next question opens as soon as one closes).
-- owner_next: only the owner launches the next question. next_delay: seconds to wait after a close (0 = none).
ALTER TABLE groups ADD COLUMN owner_next INTEGER NOT NULL DEFAULT 0;
ALTER TABLE groups ADD COLUMN next_delay INTEGER NOT NULL DEFAULT 0;
-- A closed question waiting for the next one: since when, and when it opens by itself (NULL = only the owner can).
ALTER TABLE groups ADD COLUMN paused_at TEXT;
ALTER TABLE groups ADD COLUMN next_open_at TEXT;
CREATE INDEX groups_next_open ON groups(next_open_at) WHERE next_open_at IS NOT NULL;
