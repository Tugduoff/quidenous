-- Home-screen widget (docs/WIDGET.md): a read-only token per device for `GET /api/widget`, minted from a
-- session and tied to it: whatever ends the session (sign-out, sign out everywhere, password change,
-- suspension, account deletion, idle purge) drops the widget token with it.
CREATE TABLE widget_tokens (
    token_hash TEXT PRIMARY KEY,
    session_hash TEXT NOT NULL REFERENCES user_tokens(token_hash) ON DELETE CASCADE,
    user_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    -- The device's FCM token: a silent data message asks the widget to refresh when a question
    -- opens or closes in one of the account's rooms. Null: the widget refreshes on its own timer.
    fcm_token TEXT,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    last_used_at TEXT
);
CREATE INDEX widget_tokens_session ON widget_tokens(session_hash);
CREATE INDEX widget_tokens_user ON widget_tokens(user_id);
