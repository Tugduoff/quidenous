-- Global question of the day (docs/GLOBAL_QUESTION.md): one deck question per UTC day and language,
-- picked lazily on the first request, the same for every room of that language.
CREATE TABLE global_questions (
    day TEXT NOT NULL,
    language TEXT NOT NULL,
    question_id INTEGER NOT NULL REFERENCES questions(id),
    text TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    PRIMARY KEY (day, language)
);

-- One vote per seat and question (day + language), for someone of the voter's room. Final.
CREATE TABLE global_votes (
    day TEXT NOT NULL,
    language TEXT NOT NULL,
    group_id INTEGER NOT NULL REFERENCES groups(id),
    voter_id INTEGER NOT NULL REFERENCES members(id),
    target_id INTEGER NOT NULL REFERENCES members(id),
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    PRIMARY KEY (day, language, voter_id)
);
CREATE INDEX global_votes_room ON global_votes(day, language, group_id);
