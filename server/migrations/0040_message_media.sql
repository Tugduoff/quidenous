-- Images and GIFs in the result chat (docs/CHAT_MEDIA.md). `kind` says what a message is; `text` keeps a
-- readable fallback for older apps, the inbox and reports ("📷 Photo · caption", "GIF"); `caption` is the
-- author's own words under the media; `media` (JSON) where it lives: an image file under
-- DATA_DIR/chat/<room>/ or a Klipy GIF's URLs. Existing messages are text.
ALTER TABLE messages ADD COLUMN kind TEXT NOT NULL DEFAULT 'text' CHECK (kind IN ('text', 'image', 'gif'));
ALTER TABLE messages ADD COLUMN caption TEXT;
ALTER TABLE messages ADD COLUMN media TEXT;
