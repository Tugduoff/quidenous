-- Game flow (docs/ROOMS.md, "When someone is away"): when each seat last used its room, so an owner
-- who is gone can be replaced and "only the owner launches the next one" stops waiting for them.
-- Existing seats start as active now: nobody loses anything the day this ships.
ALTER TABLE members ADD COLUMN active_at TEXT;
UPDATE members SET active_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE removed_at IS NULL;

-- Chat: the app's own id for a message, so a send retried after a lost response posts it once.
ALTER TABLE messages ADD COLUMN client_key TEXT;
CREATE UNIQUE INDEX messages_client_key ON messages(member_id, client_key) WHERE client_key IS NOT NULL;

-- Inbox gets a kind ("You are now the owner of Y", to the heir): SQLite can't change a CHECK, so
-- rebuild the table (as in 0031).
CREATE TABLE inbox_new (
    id INTEGER PRIMARY KEY,
    member_id INTEGER NOT NULL REFERENCES members(id),
    kind TEXT NOT NULL CHECK (kind IN ('message', 'nudge', 'result', 'question', 'added', 'ask', 'mention',
                                       'friend_request', 'invite', 'owner')),
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
