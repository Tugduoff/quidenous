-- Accounts you can sign in to (email + password, Google, Discord), 2FA, session metadata.
-- Legacy accounts (token only) are untouched: every column is NULL for them, every token keeps working.
ALTER TABLE users ADD COLUMN email TEXT;
ALTER TABLE users ADD COLUMN password_hash TEXT;
ALTER TABLE users ADD COLUMN email_verified_at TEXT;
-- AES-256-GCM sealed (nonce || ciphertext, base64), key derived from AUTH_SECRET.
ALTER TABLE users ADD COLUMN totp_secret TEXT;
ALTER TABLE users ADD COLUMN totp_pending TEXT;
ALTER TABLE users ADD COLUMN totp_last_step INTEGER;
CREATE UNIQUE INDEX users_email ON users(email) WHERE email IS NOT NULL;

CREATE TABLE recovery_codes (
    id INTEGER PRIMARY KEY,
    user_id INTEGER NOT NULL REFERENCES users(id),
    code_hash TEXT NOT NULL,
    used_at TEXT,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);
CREATE INDEX recovery_codes_user ON recovery_codes(user_id);

CREATE TABLE oauth_identities (
    id INTEGER PRIMARY KEY,
    user_id INTEGER NOT NULL REFERENCES users(id),
    provider TEXT NOT NULL,
    subject TEXT NOT NULL,
    email TEXT,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    UNIQUE (provider, subject)
);
CREATE INDEX oauth_identities_user ON oauth_identities(user_id);

-- Short-lived secrets, only their keyed hash is stored. expires_at / used_at are unix seconds.
CREATE TABLE auth_codes (
    id INTEGER PRIMARY KEY,
    kind TEXT NOT NULL CHECK (kind IN ('verify_email', 'reset_password', 'mfa', 'oauth_state', 'oauth_result')),
    code_hash TEXT NOT NULL,
    user_id INTEGER REFERENCES users(id),
    email TEXT,
    data TEXT,
    expires_at INTEGER NOT NULL,
    used_at INTEGER,
    attempts INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);
CREATE INDEX auth_codes_hash ON auth_codes(kind, code_hash);
CREATE INDEX auth_codes_user ON auth_codes(kind, user_id);
CREATE INDEX auth_codes_email ON auth_codes(kind, email);

-- Sessions: user_tokens already has created_at (0008).
ALTER TABLE user_tokens ADD COLUMN last_used_at TEXT;
ALTER TABLE user_tokens ADD COLUMN device TEXT;
CREATE INDEX user_tokens_user ON user_tokens(user_id);
