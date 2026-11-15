-- Indexes for the hottest lookups (before this, each of them scanned its whole table).
-- "Has this room asked this question?" (next question, queue, delete, shuffle, reorder, play).
CREATE INDEX IF NOT EXISTS rounds_group_question ON rounds(group_id, question_id);
-- A room's rounds in order (history, unseen result, party view, latest round).
CREATE INDEX IF NOT EXISTS rounds_group_number ON rounds(group_id, number);
-- A room's members (active count in every view and vote).
CREATE INDEX IF NOT EXISTS members_group ON members(group_id);
-- A room's own questions (next question, queue, import / pack duplicates).
CREATE INDEX IF NOT EXISTS questions_group ON questions(group_id);
-- Every push looks up the devices of an account.
CREATE INDEX IF NOT EXISTS push_subscriptions_user ON push_subscriptions(user_id);
CREATE INDEX IF NOT EXISTS shares_round ON shares(round_id);
CREATE INDEX IF NOT EXISTS challenges_question ON challenges(question_id);
-- Unread counts (rooms list, inbox badge): only the unread rows are indexed.
CREATE INDEX IF NOT EXISTS inbox_unread ON inbox(member_id) WHERE read_at IS NULL;

-- Deck questions are unique per language, not across languages (an English suggestion may read
-- like a French deck line).
DROP INDEX IF EXISTS questions_deck_text;
CREATE UNIQUE INDEX questions_deck_text ON questions(text, lang) WHERE group_id IS NULL;
