-- What an account wants pushed (the inbox still records everything).
CREATE TABLE notification_prefs (
    user_id INTEGER PRIMARY KEY REFERENCES users(id),
    -- Master switch, every device of the account.
    enabled INTEGER NOT NULL DEFAULT 1,
    -- Comma-separated kinds turned off: question, result, chat, nudge, added, ask.
    off TEXT NOT NULL DEFAULT '',
    -- Quiet hours, minutes after local midnight; both NULL = none. May wrap past midnight.
    quiet_start INTEGER,
    quiet_end INTEGER,
    -- Minutes east of UTC, sent by the app.
    tz_offset INTEGER NOT NULL DEFAULT 0
);

-- No push at all from this room for this seat.
ALTER TABLE members ADD COLUMN muted INTEGER NOT NULL DEFAULT 0;
