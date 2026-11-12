-- Order of a room's upcoming questions. NULL means by id (insertion order); a shuffle
-- permutes the keys of the questions not asked yet.
ALTER TABLE questions ADD COLUMN sort_key INTEGER;
