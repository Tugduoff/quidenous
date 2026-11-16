-- Consent (docs/ROOMS.md, Friends): a friend code or a "keep" asks first, and a room add is an
-- invitation the friend accepts. Existing friendships (`friends`) stay as they are.

-- A pending friend request: `from_id` asked `to_id`. Accepting turns it into two `friends` rows.
CREATE TABLE friend_requests (
    from_id INTEGER NOT NULL REFERENCES users(id),
    to_id INTEGER NOT NULL REFERENCES users(id),
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    PRIMARY KEY (from_id, to_id)
) WITHOUT ROWID;
CREATE INDEX friend_requests_to ON friend_requests(to_id);

-- A pending room invitation: `from_id` invited `user_id` into `group_id` (a room, not a party).
CREATE TABLE room_invites (
    id INTEGER PRIMARY KEY,
    group_id INTEGER NOT NULL REFERENCES groups(id),
    user_id INTEGER NOT NULL REFERENCES users(id),
    from_id INTEGER NOT NULL REFERENCES users(id),
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    UNIQUE (group_id, user_id)
);
CREATE INDEX room_invites_user ON room_invites(user_id);

-- The guest purge looks for guests (few) by this flag.
CREATE INDEX users_guest ON users(id) WHERE guest = 1 AND deleted_at IS NULL;

-- Inbox gets two kinds ("X wants to be friends", "X invited you to Y"): SQLite can't change a
-- CHECK, so rebuild the table (as in 0021).
CREATE TABLE inbox_new (
    id INTEGER PRIMARY KEY,
    member_id INTEGER NOT NULL REFERENCES members(id),
    kind TEXT NOT NULL CHECK (kind IN ('message', 'nudge', 'result', 'question', 'added', 'ask', 'mention',
                                       'friend_request', 'invite')),
    round_id INTEGER REFERENCES rounds(id),
    actor_id INTEGER REFERENCES members(id),
    text TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    read_at TEXT
);
INSERT INTO inbox_new SELECT id, member_id, kind, round_id, actor_id, text, created_at, read_at FROM inbox;
DROP TABLE inbox;
ALTER TABLE inbox_new RENAME TO inbox;
CREATE INDEX inbox_member ON inbox(member_id, id);
CREATE INDEX inbox_unread ON inbox(member_id) WHERE read_at IS NULL;
