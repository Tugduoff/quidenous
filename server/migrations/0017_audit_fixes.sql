-- How a seat ended: 1 = the person left (or deleted their account), 0 = removed (owner or vote).
-- Only someone who left can come back with the invite code. Past seats count as removals.
ALTER TABLE members ADD COLUMN left_by_self INTEGER NOT NULL DEFAULT 0;

-- Where a room question comes from: written in the app, installed from a pack, or imported from a file.
-- Only written ones count for "la plume".
ALTER TABLE questions ADD COLUMN origin TEXT NOT NULL DEFAULT 'written' CHECK (origin IN ('written', 'pack', 'import'));

-- Past questions: nobody writes three within two seconds, so a burst is an install or an import
-- (an install when the text is in a pack).
UPDATE questions SET origin = CASE
        WHEN EXISTS (SELECT 1 FROM pack_questions p WHERE lower(p.text) = lower(questions.text)) THEN 'pack'
        ELSE 'import' END
WHERE group_id IS NOT NULL AND author_id IS NOT NULL AND (
    SELECT COUNT(*) FROM questions o
    WHERE o.group_id = questions.group_id AND o.author_id = questions.author_id AND o.id != questions.id
      AND abs(julianday(o.created_at) - julianday(questions.created_at)) * 86400.0 < 2
) >= 2;
