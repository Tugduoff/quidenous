-- Anonymous votes (docs/ANONYMOUS.md). A room's rule: 'never', 'per_question' (each question says,
-- the default) or 'always'. A question (and a pack question) can ask to be anonymous; the round
-- records what was decided when it opened, so a later settings change never reveals a past vote.
ALTER TABLE groups ADD COLUMN anonymous_votes TEXT NOT NULL DEFAULT 'per_question'
    CHECK (anonymous_votes IN ('never', 'per_question', 'always'));
ALTER TABLE questions ADD COLUMN anonymous INTEGER NOT NULL DEFAULT 0;
ALTER TABLE pack_questions ADD COLUMN anonymous INTEGER NOT NULL DEFAULT 0;
ALTER TABLE rounds ADD COLUMN anonymous INTEGER NOT NULL DEFAULT 0;
