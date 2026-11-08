-- One member can be signed in on several devices (phone + web), and can sign in again after a reinstall.
CREATE TABLE member_tokens (
    token_hash TEXT PRIMARY KEY,
    member_id INTEGER NOT NULL REFERENCES members(id),
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);
INSERT INTO member_tokens (token_hash, member_id) SELECT token_hash, id FROM members;
