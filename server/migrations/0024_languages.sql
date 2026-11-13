-- French and English. `users.locale`: the language the app shows (resolved from `auto`), the one
-- the server writes push, inbox, e-mails and share cards in; `locale_pref`: the Profile setting
-- (`auto` | `fr` | `en`). Both null = never set = French (older apps).
ALTER TABLE users ADD COLUMN locale TEXT;
ALTER TABLE users ADD COLUMN locale_pref TEXT;
-- The language of the built-in deck questions a room draws (its own questions and packs: any).
ALTER TABLE groups ADD COLUMN language TEXT NOT NULL DEFAULT 'fr';
ALTER TABLE packs ADD COLUMN language TEXT NOT NULL DEFAULT 'fr';
-- Language of a built-in deck question (deck/fr.txt or deck/en.txt); room questions keep 'fr', unused.
ALTER TABLE questions ADD COLUMN lang TEXT NOT NULL DEFAULT 'fr';
-- Share cards and pages speak the sharer's language at share time.
ALTER TABLE shares ADD COLUMN lang TEXT NOT NULL DEFAULT 'fr';
ALTER TABLE stat_shares ADD COLUMN lang TEXT NOT NULL DEFAULT 'fr';
