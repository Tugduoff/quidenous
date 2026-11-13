-- no-transaction
-- A third kind of question: 'choose' (each voter wires every member to one of 2-4 items, see docs/CHOOSE.md).
-- SQLite can't change a CHECK in place: rebuild `questions` (foreign keys off for the swap, as SQLite documents).
PRAGMA foreign_keys = OFF;
BEGIN;
CREATE TABLE questions_new (
    id INTEGER PRIMARY KEY,
    group_id INTEGER REFERENCES groups(id),
    text TEXT NOT NULL,
    author_id INTEGER REFERENCES members(id),
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    kind TEXT NOT NULL DEFAULT 'pick' CHECK (kind IN ('pick', 'rank', 'choose')),
    sort_key INTEGER,
    origin TEXT NOT NULL DEFAULT 'written' CHECK (origin IN ('written', 'pack', 'import')),
    vibes TEXT,
    lang TEXT NOT NULL DEFAULT 'fr',
    suggested_by INTEGER REFERENCES users(id),
    -- choose questions: the items, a JSON array of strings.
    items TEXT
);
INSERT INTO questions_new (id, group_id, text, author_id, created_at, kind, sort_key, origin, vibes, lang, suggested_by)
SELECT id, group_id, text, author_id, created_at, kind, sort_key, origin, vibes, lang, suggested_by FROM questions;
DROP TABLE questions;
ALTER TABLE questions_new RENAME TO questions;
CREATE UNIQUE INDEX questions_deck_text ON questions(text) WHERE group_id IS NULL;

-- One row per voter and member wired: `item` is the index in questions.items.
CREATE TABLE choices (
    round_id INTEGER NOT NULL REFERENCES rounds(id),
    voter_id INTEGER NOT NULL REFERENCES members(id),
    member_id INTEGER NOT NULL REFERENCES members(id),
    item INTEGER NOT NULL,
    PRIMARY KEY (round_id, voter_id, member_id)
);
COMMIT;
PRAGMA foreign_keys = ON;
