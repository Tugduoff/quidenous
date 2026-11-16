-- Account security residuals (limits review 2026-11-14, batch 3), see docs/AUTH.md.

-- When the current e-mail was set: an address nobody verified is freed 7 days later, so a typo
-- (or someone typing my address) can't block the real owner. Kept up to date by the triggers;
-- addresses already there count from now.
ALTER TABLE users ADD COLUMN email_set_at TEXT;
UPDATE users SET email_set_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE email IS NOT NULL;
CREATE TRIGGER IF NOT EXISTS users_email_set_insert AFTER INSERT ON users
WHEN NEW.email IS NOT NULL
BEGIN
    UPDATE users SET email_set_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE id = NEW.id;
END;
CREATE TRIGGER IF NOT EXISTS users_email_set_update AFTER UPDATE OF email ON users
WHEN NEW.email IS NOT OLD.email
BEGIN
    UPDATE users SET email_set_at = CASE WHEN NEW.email IS NULL THEN NULL
        ELSE strftime('%Y-%m-%dT%H:%M:%fZ', 'now') END WHERE id = NEW.id;
END;

-- 2FA lost without recovery codes: once a code sent to the (verified) e-mail is confirmed, 2FA is
-- removed at this time (unix seconds) unless a signed-in session cancels it.
ALTER TABLE users ADD COLUMN mfa_reset_at INTEGER;

-- Two new code kinds: 'change_email' (sent to the new address) and 'mfa_reset'. SQLite can't
-- change a CHECK, so rebuild the table (nothing references it), as in 0016.
CREATE TABLE auth_codes_new (
    id INTEGER PRIMARY KEY,
    kind TEXT NOT NULL CHECK (kind IN ('verify_email', 'reset_password', 'mfa', 'oauth_state',
        'oauth_result', 'delete_account', 'change_email', 'mfa_reset')),
    code_hash TEXT NOT NULL,
    user_id INTEGER REFERENCES users(id),
    email TEXT,
    data TEXT,
    expires_at INTEGER NOT NULL,
    used_at INTEGER,
    attempts INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);
INSERT INTO auth_codes_new (id, kind, code_hash, user_id, email, data, expires_at, used_at, attempts, created_at)
    SELECT id, kind, code_hash, user_id, email, data, expires_at, used_at, attempts, created_at FROM auth_codes;
DROP TABLE auth_codes;
ALTER TABLE auth_codes_new RENAME TO auth_codes;
CREATE INDEX auth_codes_hash ON auth_codes(kind, code_hash);
CREATE INDEX auth_codes_user ON auth_codes(kind, user_id);
CREATE INDEX auth_codes_email ON auth_codes(kind, email);
