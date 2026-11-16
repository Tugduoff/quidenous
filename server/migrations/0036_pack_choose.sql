-- "Qui choisirait quoi" questions in packs: a pack question's answers (JSON array, 2 to 4), like questions.items.
ALTER TABLE pack_questions ADD COLUMN items TEXT;
