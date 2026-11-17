-- A double tap on "Accepter" (docs/ROOMS.md, Friends): the second tap gets the seat the first one took, not a 404.
-- So an accepted invitation is kept, with that seat, instead of being deleted. At most one per room and person
-- (UNIQUE (group_id, user_id)): the next invitation into that room replaces it. NULL = still waiting for an answer.
ALTER TABLE room_invites ADD COLUMN member_id INTEGER REFERENCES members(id);
