-- no-transaction
-- « Qui a dit ça ? » questions (docs/SAID.md): a fourth kind, 'said'. Everyone writes a short answer,
-- then everyone matches the answers to their authors.
-- SQLite can't change a CHECK in place: rebuild `questions` and `reports` (foreign keys off for the
-- swap, as SQLite documents and as in 0027). Atomic: `db::NO_TX_PROBES` recognises the finished state.
PRAGMA foreign_keys = OFF;
BEGIN;
CREATE TABLE questions_new (
    id INTEGER PRIMARY KEY,
    group_id INTEGER REFERENCES groups(id),
    text TEXT NOT NULL,
    author_id INTEGER REFERENCES members(id),
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    kind TEXT NOT NULL DEFAULT 'pick' CHECK (kind IN ('pick', 'rank', 'choose', 'said')),
    sort_key INTEGER,
    origin TEXT NOT NULL DEFAULT 'written' CHECK (origin IN ('written', 'pack', 'import')),
    vibes TEXT,
    lang TEXT NOT NULL DEFAULT 'fr',
    suggested_by INTEGER REFERENCES users(id),
    -- choose questions: the items, a JSON array of strings.
    items TEXT,
    anonymous INTEGER NOT NULL DEFAULT 0
);
INSERT INTO questions_new (id, group_id, text, author_id, created_at, kind, sort_key, origin, vibes, lang, suggested_by, items, anonymous)
SELECT id, group_id, text, author_id, created_at, kind, sort_key, origin, vibes, lang, suggested_by, items, anonymous FROM questions;
DROP TABLE questions;
ALTER TABLE questions_new RENAME TO questions;
CREATE INDEX questions_group ON questions(group_id);
CREATE UNIQUE INDEX questions_deck_text ON questions(text, lang) WHERE group_id IS NULL;

-- Reports get a fifth kind: an answer to a said question.
CREATE TABLE reports_new (
    id INTEGER PRIMARY KEY,
    reporter_id INTEGER NOT NULL REFERENCES users(id),
    kind TEXT NOT NULL CHECK (kind IN ('message', 'member', 'question', 'suggestion', 'answer')),
    target_id INTEGER NOT NULL,
    room_id INTEGER REFERENCES groups(id),
    reported_user_id INTEGER REFERENCES users(id),
    content TEXT NOT NULL,
    reason TEXT NOT NULL CHECK (reason IN ('offensive', 'harassment', 'sexual', 'spam', 'other')),
    details TEXT,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    -- When an admin closed it, and how: dismiss, delete, suspend, erase.
    resolved_at TEXT,
    resolution TEXT
);
INSERT INTO reports_new (id, reporter_id, kind, target_id, room_id, reported_user_id, content, reason, details, created_at, resolved_at, resolution)
SELECT id, reporter_id, kind, target_id, room_id, reported_user_id, content, reason, details, created_at, resolved_at, resolution FROM reports;
DROP TABLE reports;
ALTER TABLE reports_new RENAME TO reports;
CREATE UNIQUE INDEX reports_once ON reports(reporter_id, kind, target_id);
CREATE INDEX reports_open ON reports(kind, target_id) WHERE resolved_at IS NULL;

-- When the guessing started (null while people write, and for other kinds).
ALTER TABLE rounds ADD COLUMN said_guess_at TEXT;

-- One answer per member and question. `id` is random (never sequential: neither the ids nor their
-- order say who wrote first). `text` null = a pass from an app too old to write (written, not
-- guessable). `hidden_at`: hidden by reports; an admin's delete also blanks `text` ('').
CREATE TABLE said_answers (
    id INTEGER PRIMARY KEY,
    round_id INTEGER NOT NULL REFERENCES rounds(id) ON DELETE CASCADE,
    member_id INTEGER NOT NULL REFERENCES members(id),
    text TEXT,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    hidden_at TEXT,
    UNIQUE (round_id, member_id)
);

-- A voter's matching: answer → the member they think wrote it.
CREATE TABLE said_guesses (
    round_id INTEGER NOT NULL REFERENCES rounds(id) ON DELETE CASCADE,
    voter_id INTEGER NOT NULL REFERENCES members(id),
    answer_id INTEGER NOT NULL REFERENCES said_answers(id) ON DELETE CASCADE,
    member_id INTEGER NOT NULL REFERENCES members(id),
    PRIMARY KEY (round_id, voter_id, answer_id)
);
CREATE INDEX said_guesses_answer ON said_guesses(answer_id);
COMMIT;
PRAGMA foreign_keys = ON;
