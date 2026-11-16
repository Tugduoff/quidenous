-- A push device belongs to the session that registered it: signing out, being signed out
-- (revoke, revoke others, password reset) or deleting the account stops its notifications.
ALTER TABLE push_subscriptions ADD COLUMN token_hash TEXT;
-- Devices registered before this: the account's session when it has only one.
UPDATE push_subscriptions SET token_hash = (
    SELECT t.token_hash FROM user_tokens t WHERE t.user_id = push_subscriptions.user_id
) WHERE (SELECT COUNT(*) FROM user_tokens t WHERE t.user_id = push_subscriptions.user_id) = 1;
CREATE INDEX IF NOT EXISTS push_subscriptions_token ON push_subscriptions(token_hash);
-- The others (unknown session) go as soon as any session of the account ends; the app registers
-- its device again at its next start, this time with its session.
CREATE TRIGGER IF NOT EXISTS push_subscriptions_follow_session AFTER DELETE ON user_tokens
BEGIN
    DELETE FROM push_subscriptions
    WHERE token_hash = OLD.token_hash OR (token_hash IS NULL AND user_id = OLD.user_id);
END;
