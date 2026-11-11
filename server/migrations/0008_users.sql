-- One account per person, in as many rooms as they like. Existing members each become a user
-- (same id), their device tokens move over unchanged, so nobody is signed out.
CREATE TABLE users (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL,
    color TEXT NOT NULL,
    photo TEXT,
    friend_code TEXT NOT NULL UNIQUE,
    last_group_id INTEGER REFERENCES groups(id),
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);
INSERT INTO users (id, name, color, photo, friend_code, last_group_id, created_at)
    SELECT id, name, color, photo, upper(hex(randomblob(4))), group_id, created_at FROM members;

CREATE TABLE user_tokens (
    token_hash TEXT PRIMARY KEY,
    user_id INTEGER NOT NULL REFERENCES users(id),
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);
INSERT INTO user_tokens (token_hash, user_id, created_at)
    SELECT t.token_hash, t.member_id, t.created_at FROM member_tokens t JOIN members m ON m.id = t.member_id;

ALTER TABLE members ADD COLUMN user_id INTEGER REFERENCES users(id);
UPDATE members SET user_id = id;
CREATE UNIQUE INDEX members_user_group ON members(user_id, group_id);

-- Owner of each existing room: its earliest active member.
ALTER TABLE members ADD COLUMN role TEXT NOT NULL DEFAULT 'member' CHECK (role IN ('owner', 'member'));
UPDATE members SET role = 'owner'
    WHERE id IN (SELECT MIN(id) FROM members WHERE removed_at IS NULL GROUP BY group_id);

-- Room settings, owner only.
ALTER TABLE groups ADD COLUMN skip_vote INTEGER NOT NULL DEFAULT 1;
ALTER TABLE groups ADD COLUMN kick_vote INTEGER NOT NULL DEFAULT 1;
ALTER TABLE groups ADD COLUMN questions_by TEXT NOT NULL DEFAULT 'everyone' CHECK (questions_by IN ('everyone', 'owner'));

-- Push goes to people, not to their seat in one room.
ALTER TABLE push_subscriptions ADD COLUMN user_id INTEGER REFERENCES users(id);
UPDATE push_subscriptions SET user_id = member_id;
