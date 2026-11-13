-- Community v1 (docs/COMMUNITY.md): packs 2.0 (categories, 16+, likes, installs per room, moderation
-- statuses, editing), suggestions to the official deck, share → play and challenges between rooms.

-- Packs: categories (comma-separated ids, max 3), 16+, Coflop's selection, a status instead of `hidden`.
ALTER TABLE packs ADD COLUMN tags TEXT NOT NULL DEFAULT '';
ALTER TABLE packs ADD COLUMN spicy INTEGER NOT NULL DEFAULT 0;
ALTER TABLE packs ADD COLUMN featured INTEGER NOT NULL DEFAULT 0;
ALTER TABLE packs ADD COLUMN status TEXT NOT NULL DEFAULT 'public'
    CHECK (status IN ('public', 'review', 'removed', 'deleted'));
ALTER TABLE packs ADD COLUMN removed_reason TEXT;
ALTER TABLE packs ADD COLUMN version INTEGER NOT NULL DEFAULT 1;
ALTER TABLE packs ADD COLUMN updated_at TEXT;
-- `hidden` is no longer read: hidden by reports → waits for review, hidden by its author → deleted.
UPDATE packs SET status = CASE
        WHEN (SELECT COUNT(*) FROM pack_reports r WHERE r.pack_id = packs.id) >= 3 THEN 'review'
        ELSE 'deleted' END
WHERE hidden = 1;

-- One install per pack and room (installing again only tops the room up).
CREATE TABLE pack_installs (
    pack_id INTEGER NOT NULL REFERENCES packs(id),
    group_id INTEGER NOT NULL REFERENCES groups(id),
    user_id INTEGER REFERENCES users(id),
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    PRIMARY KEY (pack_id, group_id)
);
CREATE INDEX pack_installs_recent ON pack_installs(created_at);
-- Past installs: room questions that came from a pack, one row per pack and room, earliest date
-- (bare columns of a MIN() aggregate come from the row holding the minimum).
INSERT OR IGNORE INTO pack_installs (pack_id, group_id, user_id, created_at)
    SELECT pq.pack_id, q.group_id, m.user_id, MIN(q.created_at)
    FROM questions q
    JOIN pack_questions pq ON lower(pq.text) = lower(q.text)
    LEFT JOIN members m ON m.id = q.author_id
    WHERE q.origin = 'pack' AND q.group_id IS NOT NULL
    GROUP BY pq.pack_id, q.group_id;
UPDATE packs SET installs = (SELECT COUNT(*) FROM pack_installs i WHERE i.pack_id = packs.id);

CREATE TABLE pack_likes (
    pack_id INTEGER NOT NULL REFERENCES packs(id),
    user_id INTEGER NOT NULL REFERENCES users(id),
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    PRIMARY KEY (pack_id, user_id)
);
CREATE INDEX pack_likes_recent ON pack_likes(created_at);

-- Why a pack was reported (older reports have none). ALTER can't take an expression default:
-- the server sets `created_at`.
ALTER TABLE pack_reports ADD COLUMN reason TEXT;
ALTER TABLE pack_reports ADD COLUMN details TEXT;
ALTER TABLE pack_reports ADD COLUMN created_at TEXT;

-- Opt-in to 16+ packs in the store.
ALTER TABLE users ADD COLUMN show_spicy INTEGER NOT NULL DEFAULT 0;

-- Questions proposed for the official deck: voted on by everyone, then approved by an admin.
CREATE TABLE suggestions (
    id INTEGER PRIMARY KEY,
    user_id INTEGER NOT NULL REFERENCES users(id),
    text TEXT NOT NULL,
    kind TEXT NOT NULL DEFAULT 'pick' CHECK (kind IN ('pick', 'rank')),
    lang TEXT NOT NULL DEFAULT 'fr',
    status TEXT NOT NULL DEFAULT 'voting' CHECK (status IN ('voting', 'queued', 'approved', 'rejected')),
    up INTEGER NOT NULL DEFAULT 0,
    down INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    decided_at TEXT,
    question_id INTEGER REFERENCES questions(id)
);
CREATE INDEX suggestions_open ON suggestions(status, lang, id);
CREATE INDEX suggestions_user ON suggestions(user_id, created_at);
-- 1 up, -1 down, 0 skipped.
CREATE TABLE suggestion_votes (
    suggestion_id INTEGER NOT NULL REFERENCES suggestions(id),
    user_id INTEGER NOT NULL REFERENCES users(id),
    value INTEGER NOT NULL CHECK (value IN (-1, 0, 1)),
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    PRIMARY KEY (suggestion_id, user_id)
);
-- A shared-deck question that came from an approved suggestion (never moved out of the deck).
ALTER TABLE questions ADD COLUMN suggested_by INTEGER REFERENCES users(id);

-- Round `from_round_id` sent to room `to_group_id` as its question `question_id`.
CREATE TABLE challenges (
    id INTEGER PRIMARY KEY,
    from_round_id INTEGER NOT NULL REFERENCES rounds(id),
    to_group_id INTEGER NOT NULL REFERENCES groups(id),
    question_id INTEGER NOT NULL REFERENCES questions(id),
    created_by INTEGER REFERENCES users(id),
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);
CREATE UNIQUE INDEX challenges_once ON challenges(from_round_id, to_group_id);
CREATE INDEX challenges_to ON challenges(to_group_id, question_id);
