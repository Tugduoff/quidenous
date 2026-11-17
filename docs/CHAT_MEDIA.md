# Chat images and GIFs

Photos and GIFs in the chat under a finished question (code: `server/src/media.rs`, `server/src/gifs.rs`; migration `0040`). Features: `chat_images` (always on), `chat_gifs` (only when `KLIPY_API_KEY` is set). The app hides each picker when its feature is absent.

## Messages

`messages` gained `kind` (`text` | `image` | `gif`), `caption` (the author's words under the media, 500 characters at most, `400 bad_caption`) and `media` (JSON, where it lives). Every message in `GET /rounds/{id}/messages` and the `POST` answers carries:

- `kind`, `caption` (null for text), `media` (null for text):
  - image: `{ "url": "/api/media/<token>", "width", "height", "mime": "image/jpeg" | "image/png" | "image/gif" }`
  - GIF: `{ "url": "/api/gifs/media/<token>", "preview_url": "/api/gifs/media/<token>", "width", "height", "mime": "image/webp" | "image/gif", "provider": "klipy", "id" }`
  - URLs are paths on this server: the app puts its API origin in front.
- `text`, still filled for older apps (they show it as a plain message): `📷 Photo` or `GIF`, plus ` · <caption>` when there is one. The same text goes to the inbox, pushes and report snapshots (identical in French and English: it is stored once in `messages.text`). Apps should treat an unknown `kind` as text and show `text`.
- Mentions and `@tous` work on the caption exactly as on a text message (`mentions`, `everyone`; one shared code path, `routes::post_chat`).

Same rules as text: a finished question of my room only (`400 round_open` while open, `404` for another room), the chat limits (15 / minute, 300 / hour), inbox entries, pushes, live `message_posted`. Reactions, deleting (author, owner, admin), reports (three reporters hide it), blocks (hidden from whoever blocked the author) all apply as to text.

## Images

`POST /api/rounds/{id}/messages/image`, `multipart/form-data`:

| Field | |
| --- | --- |
| `image` | the file (required, `400 missing_image`) |
| `caption` | optional text |
| `mentions` | optional JSON array of member ids, e.g. `[12,15]` |
| `everyone` | optional, `1` for `@tous` |

- The type comes from the file's first bytes, never its name or declared type: JPEG, PNG, WebP or GIF; anything else (text, SVG, HEIC…) or an image that doesn't decode is `400 bad_image`. Over 8 MB: `413 too_large`. Decoding is limited to 4096 px a side and 64 MB (decompression bombs; the app sends 1600 px), off the async threads, and at most 2 image decodes run at once server-wide (chat and profile photos together; the others wait).
- Metadata is gone by construction: JPEG and WebP are decoded, turned upright (EXIF orientation), scaled to 1600 px on the longest side at most and re-encoded as JPEG (quality 85); a WebP with transparency and a PNG become a PNG the same way. A GIF is kept byte for byte (animated; GIF has no EXIF) once its first frame decodes.
- Limits on top of the chat ones: 20 images / hour and 100 / day per account (`429 rate_limited`), taken before the body is read (a retry with the same `client_key` gives them back).
- Stored as `DATA_DIR/chat/<room id>/<128-bit random hex>.<jpg|png|gif>`; `media` keeps the file name, size and type.

### Signed URLs

`media.url` is `/api/media/<token>`: `base64url("<message id>.<expiry>")` + `.` + an HMAC-SHA256 (128 bits) keyed by the server secret (`AUTH_SECRET` or `auth.key`, through the auth key derivation). Only members who can read the chat get one. The expiry is the next 6-hour boundary plus 6 hours (6 to 12 hours of life), so the URL stays the same for hours and image caches keep working; the app gets fresh ones with every messages fetch.

`GET /api/media/{token}` needs no sign-in (image tags can't send a header; the token is the key). It answers the file with its type, `Cache-Control: private, max-age=<seconds left>`, `X-Content-Type-Options: nosniff`, `Content-Security-Policy: default-src 'none'; sandbox` and `Content-Disposition: inline; filename="quidenous-<id>.<ext>"`, and `404` for a token that is forged, expired, or for a message that is gone, not an image, hidden by reports, or in a deleted room. The web app's CSP already allows same-origin images (`img-src 'self' data: blob:`).

Admins see reported images and GIFs in `GET /admin/review` (`media` on message reports: `{kind, url, …}`); the image URL there also works while the message is hidden.

## GIFs (Klipy)

The server searches Klipy and proxies every GIF file: phones never talk to Klipy (no IP, no key on the device). Without `KLIPY_API_KEY`, `GET /api/gifs` and `POST …/messages/gif` answer `404 gifs_off`.

- `GET /api/gifs?q=<text>&page=<n>` (signed in): a page of 24, trending when `q` is empty (100 characters kept, pages 1-50). Answers `{ "items": [{ "id", "title", "width", "height", "preview_url", "url" }], "next": bool }`, URLs on our proxy. The server asks Klipy with the reader's region (`FR` / `GB`), `KLIPY_CONTENT_FILTER` and an opaque per-account `customer_id` (an HMAC, not the account id). Renditions: `url` = medium animated WebP (else GIF, else HD), `preview_url` = small (else extra-small, else medium); MP4 is never used (it would need a video player). Ads and items without a file on Klipy's static hosts are left out. 60 searches / minute per account; Klipy down or slow (5 s): `503 gifs_unavailable`.
- `GET /api/gifs/media/{token}` (no sign-in): `token` signs the original URL, which must be `https://static*.klipy.com`. The file is fetched and streamed back (`image/gif` or `image/webp` only, 10 MB at most, `Cache-Control: public, max-age=86400`), never stored. Anything else is `404`. It keeps working without the key, so GIFs already posted still show.
- `POST /api/rounds/{id}/messages/gif` JSON `{ "id", "url", "preview_url"?, "width", "height", "caption"?, "mentions"?, "everyone"? }` with `url` / `preview_url` exactly as `/api/gifs` gave them (relative or absolute). The server checks their signatures and stores the original Klipy URLs and id (`400 bad_gif` for a URL it didn't sign, a size outside 1-4096 or a missing id); the answer re-signs them.
- The key goes in the URL path of Klipy's API: it is never logged (request errors are logged without their URL).
- Klipy's rules ask for attribution: the app's search field reads "Rechercher sur KLIPY" / "Search KLIPY".

### Setup

1. Create an app in Klipy's partner dashboard (klipy.com) and copy its API key.
2. `KLIPY_API_KEY=<key>` in `deploy/.env` (optionally `KLIPY_CONTENT_FILTER=off|low|medium|high`, default `low` ≈ PG-13), then `docker compose up -d`.
3. `GET /api/features` lists `chat_gifs`.

## Retention and disk

An image stays until its message is deleted (author, owner, admin), its author deletes their account (guests purged with them), or its room is deleted (the whole `chat/<room>/` folder goes: rooms are never restored). Hidden and blocked messages keep their file. `chat/` is in the nightly backup (`deploy/backup.sh`).

Disk: a resized photo is about 200-400 KB (GIFs uploaded as files keep their size, up to 8 MB). Ten photos a day in a room is roughly 1-1.5 GB a year for that room; Klipy GIFs take no disk.
