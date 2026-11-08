CREATE TABLE groups (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL,
    join_code TEXT NOT NULL UNIQUE,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE TABLE members (
    id INTEGER PRIMARY KEY,
    group_id INTEGER NOT NULL REFERENCES groups(id),
    name TEXT NOT NULL,
    color TEXT NOT NULL,
    photo TEXT,
    token_hash TEXT NOT NULL UNIQUE,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE TABLE questions (
    id INTEGER PRIMARY KEY,
    group_id INTEGER REFERENCES groups(id),
    text TEXT NOT NULL,
    author_id INTEGER REFERENCES members(id),
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);
CREATE UNIQUE INDEX questions_deck_text ON questions(text) WHERE group_id IS NULL;

CREATE TABLE rounds (
    id INTEGER PRIMARY KEY,
    group_id INTEGER NOT NULL REFERENCES groups(id),
    question_id INTEGER NOT NULL REFERENCES questions(id),
    number INTEGER NOT NULL,
    opened_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    closed_at TEXT
);
CREATE UNIQUE INDEX rounds_one_open ON rounds(group_id) WHERE closed_at IS NULL;

CREATE TABLE votes (
    round_id INTEGER NOT NULL REFERENCES rounds(id),
    voter_id INTEGER NOT NULL REFERENCES members(id),
    target_id INTEGER NOT NULL REFERENCES members(id),
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    PRIMARY KEY (round_id, voter_id)
);

CREATE TABLE ratings (
    round_id INTEGER NOT NULL REFERENCES rounds(id),
    member_id INTEGER NOT NULL REFERENCES members(id),
    value INTEGER NOT NULL CHECK (value IN (-1, 1)),
    PRIMARY KEY (round_id, member_id)
);
