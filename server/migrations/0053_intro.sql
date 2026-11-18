-- The first-run intro (docs/TODAY.md, "Intro"): when this account finished or skipped it. NULL = never
-- shown yet. Accounts that existed before the intro never see it: they are marked done now.
ALTER TABLE users ADD COLUMN intro_done_at TEXT;
UPDATE users SET intro_done_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now');
