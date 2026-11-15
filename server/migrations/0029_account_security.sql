-- Account security (audit 2026-11-14, H4a + M2), see docs/AUTH.md.
-- Re-authentication: when this session last proved who it is (password, 2FA code or a fresh
-- Google / Discord round), unix seconds. Sensitive changes need it to be recent.
ALTER TABLE user_tokens ADD COLUMN reauth_at INTEGER;

-- Wrong six-digit codes (reset, delete, verify) per e-mail, kept across restarts: after too many
-- in 24 h, codes for that e-mail stop being accepted until the oldest failure is a day old.
CREATE TABLE code_failures (
    id INTEGER PRIMARY KEY,
    email TEXT NOT NULL,
    at INTEGER NOT NULL
);
CREATE INDEX code_failures_email_at ON code_failures(email, at);
