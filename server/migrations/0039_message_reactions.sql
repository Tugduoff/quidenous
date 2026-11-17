-- Chat reactions (docs/ROOMS.md, "Chat reactions"): one row per message, member and emoji, from a
-- fixed list (server/src/reactions.rs). They go with their message (delete, account deletion).
CREATE TABLE message_reactions (
    message_id INTEGER NOT NULL REFERENCES messages(id) ON DELETE CASCADE,
    member_id INTEGER NOT NULL REFERENCES members(id) ON DELETE CASCADE,
    emoji TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    PRIMARY KEY (message_id, member_id, emoji)
) WITHOUT ROWID;
CREATE INDEX message_reactions_member ON message_reactions(member_id);
