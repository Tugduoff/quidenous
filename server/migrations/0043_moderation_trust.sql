-- A room owner restored a message hidden by reports: only reports made after this hide it again
-- (the earlier ones stay open for the admins). NULL = never restored.
ALTER TABLE messages ADD COLUMN unhidden_at TEXT;
-- Erasing an account and the hourly guest purge delete a member's messages and the inbox entries
-- they caused: without these, both scanned the whole table under the write lock.
CREATE INDEX IF NOT EXISTS messages_member ON messages(member_id);
CREATE INDEX IF NOT EXISTS inbox_actor ON inbox(actor_id);
