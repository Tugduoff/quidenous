-- Drawn avatars: a compact spec ("0.2.5.1…", one option index per field of assets/avatar.json),
-- instead of a photo (one or the other). Copied onto a new seat like the photo.
-- stat_shares.people now freezes [first name, colour, avatar|null] (old rows keep [first name, colour]).
ALTER TABLE users ADD COLUMN avatar TEXT;
ALTER TABLE members ADD COLUMN avatar TEXT;
