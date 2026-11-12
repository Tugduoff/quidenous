-- Store readiness: deleting an account, reporting content, blocking someone.

-- A deleted account keeps its row (votes, packs and members point at it), scrubbed.
ALTER TABLE users ADD COLUMN deleted_at TEXT;

-- auth_codes gets a new kind ('delete_account', the public deletion page): SQLite can't change
-- a CHECK, so rebuild the table (nothing references it).
CREATE TABLE auth_codes_new (
    id INTEGER PRIMARY KEY,
    kind TEXT NOT NULL CHECK (kind IN ('verify_email', 'reset_password', 'mfa', 'oauth_state', 'oauth_result', 'delete_account')),
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

-- Reported messages, members and questions. `content` is a snapshot taken at report time
-- (the message can be deleted, the member renamed). One report per person and target.
CREATE TABLE reports (
    id INTEGER PRIMARY KEY,
    reporter_id INTEGER NOT NULL REFERENCES users(id),
    kind TEXT NOT NULL CHECK (kind IN ('message', 'member', 'question')),
    target_id INTEGER NOT NULL,
    room_id INTEGER REFERENCES groups(id),
    reported_user_id INTEGER REFERENCES users(id),
    content TEXT NOT NULL,
    reason TEXT NOT NULL CHECK (reason IN ('offensive', 'harassment', 'sexual', 'spam', 'other')),
    details TEXT,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);
CREATE UNIQUE INDEX reports_once ON reports(reporter_id, kind, target_id);

-- People I blocked: their messages, notifications and friend requests don't reach me.
CREATE TABLE blocks (
    user_id INTEGER NOT NULL REFERENCES users(id),
    blocked_id INTEGER NOT NULL REFERENCES users(id),
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    PRIMARY KEY (user_id, blocked_id)
);
CREATE INDEX blocks_blocked ON blocks(blocked_id);
