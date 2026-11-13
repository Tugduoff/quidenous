# Community (packs 2.0, suggestions to the deck, share → play, challenges)

Feature flag: `GET /api/features` lists `community` when the server has everything below.
All endpoints are under `/api`, authenticated with `Authorization: Bearer <token>` unless marked public.
`X-Room` (optional) is the room the app is in, as everywhere else.

Admins: `ADMIN_USER_IDS` env (comma-separated user ids). Admin-only endpoints answer `403 not_admin` to others.

## Migration `0025_community.sql`

- `packs`: `tags TEXT NOT NULL DEFAULT ''` (comma-separated category ids, max 3), `spicy INTEGER NOT NULL DEFAULT 0` (16+),
  `featured INTEGER NOT NULL DEFAULT 0`, `status TEXT NOT NULL DEFAULT 'public'` (`public` | `review` | `removed` | `deleted`),
  `removed_reason TEXT`, `version INTEGER NOT NULL DEFAULT 1`, `updated_at TEXT`.
  Backfill: `hidden = 1` with 3+ reports → `review`, other `hidden = 1` → `deleted`. `hidden` is no longer read.
- `pack_installs (pack_id, group_id, user_id, created_at)`, PK `(pack_id, group_id)`: one install per room.
  Backfill from room questions with `origin = 'pack'` whose text is in a pack (one row per pack and room, earliest date),
  then `packs.installs` = that count (cached, kept in sync on install).
- `pack_likes (pack_id, user_id, created_at)`, PK `(pack_id, user_id)`.
- `pack_reports`: `reason TEXT`, `details TEXT`, `created_at TEXT`.
- `users.show_spicy INTEGER NOT NULL DEFAULT 0` (opt-in to 16+ packs).
- `suggestions (id, user_id, text, kind 'pick'|'rank', lang, status 'voting'|'queued'|'approved'|'rejected', up, down, created_at, decided_at, question_id)`.
- `suggestion_votes (suggestion_id, user_id, value -1|0|1, created_at)`, PK `(suggestion_id, user_id)`; 0 = skipped.
- `questions.suggested_by INTEGER REFERENCES users(id)`: a shared-deck question that came from a suggestion.
  `deck::adopt_imports` must never move these out of the shared deck.
- `challenges (id, from_round_id, to_group_id, question_id, created_by, created_at)`: the round `from_round_id` was
  sent to room `to_group_id` as its question `question_id` (the target round is the round of that question in that room).

## Categories

Fixed ids (server validates, app names and icons them): `potes` (Entre potes, beer), `pimente` (Pimenté 16+, flame),
`boulot` (Boulot, briefcase), `famille` (Famille, house), `soiree` (Soirée, party-popper), `culture` (Culture, book-open),
`absurde` (Absurde, sparkles). A pack tagged `pimente` is always `spicy`.

## Packs

Pack item (lists, creator page, mine):
```
{ id, name, description, author, author_id, installs, likes, liked, count, mine, sample: [string x3],
  language, tags: [string], spicy, featured, installed, status, version, removed_reason }
```
`installed`: this pack is installed in the `X-Room` room. `status`/`removed_reason` matter only for the author (and admins).

- `GET /packs?sort=trending|new|liked|featured&tag=<id>&lang=fr|en&q=<search>` → `[item]` (max 50).
  Only `public` packs. `trending` (default): installs + likes of the last 7 days, then total installs; `new`: newest first;
  `liked`: most likes; `featured`: `featured = 1` only, newest first. `lang` absent = all languages.
  Spicy packs are listed only when the user opted in (`show_spicy`) or the `X-Room` room's vibe is `pimente`
  (or the `tag=pimente` filter is asked explicitly while opted in).
- `GET /packs/{id}` → item + `questions: [{ text, kind }]`. Visible if `public`, or to its author, or to an admin; else 404.
- `POST /packs { name, description?, language?, tags?: [id], spicy?: bool, text?: string, questions?: [{text, kind}] }`
  → `{ id, count }`. Source: `questions`, else `text` (file, `[classement]` lines), else the room's own questions.
  `400 bad_pack | pack_size | bad_language | bad_tags`, `409 too_many_packs` (10 packs in `public`/`review`; removed and deleted don't count).
- `PUT /packs/{id} { name, description, language, tags, spicy, questions: [{text, kind}] }` → item. Author only (`403 not_author`);
  not when `removed`/`deleted` (`403 pack_removed`). Changed questions → `version + 1`, `updated_at`. Same validation as publish.
- `DELETE /packs/{id}` → author: `status = 'deleted'`.
- `GET /packs/mine` → my packs that are not `deleted` (public, review, removed with its reason), newest first.
- `POST /packs/{id}/install` → `{ added, skipped, installed: true }`. Into the `X-Room` room (questions_by rule as before).
  Public packs (or the author's own). Counts once per room: `pack_installs` insert-or-ignore, `installs` recounted.
  Installing again adds only the questions the room lacks (new version).
- `POST /packs/{id}/like` / `DELETE /packs/{id}/like` → `{ likes, liked }`. Not on my own pack (`400 own_pack`).
- `POST /packs/{id}/report { reason?: 'offensive'|'sexual'|'spam'|'other', details? }` → `{ ok }`. One per person.
  The 3rd distinct report moves a `public` pack to `review`. Each new report is e-mailed to `REPORT_EMAIL`
  (contact@coflop.fr; same hourly cap as other reports) with the pack, its author, reason, report count and whether it went to review.
- `GET /creators/{user_id}` → `{ id, name, color, photo_url, avatar, installs, likes, packs: [item] }`: public packs only. 404 if none and not me.
- `GET /community/me` → `{ admin, show_spicy, suggestions_left }`; `PUT /community/me { show_spicy }` → same.

## Admin

- `GET /admin/review` → `{ packs: [item + reports: [{ reason, details, created_at }]], suggestions: [suggestion + author] }`:
  packs in `review`, suggestions in `queued`.
- `POST /admin/packs/{id}/approve` → back to `public`, its reports cleared.
- `POST /admin/packs/{id}/remove { reason }` → `removed` with the reason (the author sees it in Mes packs).
- `POST /admin/packs/{id}/feature { featured: bool }` → `{ featured }` (Sélection Coflop).
- `POST /admin/suggestions/{id}/approve { tags?: [deck tag] }` → the question joins the shared deck of its language
  (`group_id NULL`, `suggested_by` = author, vibes = tags or `potes`); `status = 'approved'`, `question_id` set.
- `POST /admin/suggestions/{id}/reject` → `rejected`.

## Suggestions to the official deck

Suggestion: `{ id, text, kind, lang, status, up, down, author, mine }`.
- `POST /suggestions { text, kind?: 'pick'|'rank', lang? }` → suggestion. 8–200 chars; 5 per user per rolling 24 h
  (`429 too_many_suggestions`); `409 duplicate` when the text (case-insensitive) is already in the deck or suggested.
  `lang` defaults to the user's locale, else `fr`.
- `GET /suggestions/next?lang=` → up to 20 `voting` suggestions in that language (default: user's locale), not mine,
  not voted (or skipped) by me, oldest first.
- `POST /suggestions/{id}/vote { value: 1 | -1 | 0 }` → `{ up, down, status }`. 0 = skip. Not my own (`400 own_suggestion`),
  only `voting` ones (`409 closed`). Changing my vote is allowed. Reaching `up - down >= 10` with `up / (up + down) >= 0.7`
  moves it to `queued` (admin approval).
- `GET /suggestions/mine` → `{ left, items: [suggestion] }` newest first.
- Round view (`GET /round/current`, `/rounds/{id}`) gains `suggested_by: string | null` (the author's name) for an approved
  suggestion: the app shows "proposée par X" on the question card.

## Share → play

- `/s/{token}` page: question shares get a primary CTA "Jouer cette question avec tes potes" (`/play?share={token}`);
  stat shares keep "Ouvrir l'app". Same languages as the page.
- `/p/{id}` (public HTML, OG tags, `/p/{id}/card.png` card): a public pack's name, description, question count,
  three sample questions, author, and the CTA "Installer dans un salon" → `/pack/{id}` (the web app). 404 when not public.
- `GET /play/{token}` (public, no auth) → `{ question, kind, lang, room, closed }`: the shared question (`closed`: its round is closed,
  so playing it makes a challenge).
- `POST /play/{token}` (auth, `X-Room` = target room) → `{ question_id, challenge_id | null }`: adds the question to the
  target room as its next question (front of the queue; reuses an unused copy with the same text). If the shared round is
  closed and the target room is another room, a challenge is recorded. `409 already_asked` if the room already played it
  (the challenge still links to that round). `403 owner_only` per `questions_by`.
- `POST /rounds/{id}/challenge { room_id }` (auth, `X-Room` = source room, round closed) → `{ challenge_id }`: same, to one
  of my other rooms (`403 not_in_room` if I am not in it, `400 same_room`).
- `GET /rounds/{id}/challenges` → `[{ room, round_id | null, closed, podium | null, mine_podium }]`: every room this round
  was sent to or came from. `podium` (top 3 of the other room: `[{ name, color, avatar, photo_url, score }]`, score = votes
  or ranking points) only once both rounds are closed; `mine_podium` same shape for this room.

## App

- Packs store: sort tabs, category chips + 16+ chip (toggles `show_spicy`), likes, installed marker, featured badge.
  Publish/edit screen, Mes packs, creator page, admin review screen, pack share link `/p/{id}`.
- Install picks a room when I have several.
- "Proposer" screen: compose (ranking toggle, x/5 left), vote card stack (up / down / skip), my suggestions.
- `/play?share=` screen: add the shared question to one of my rooms or create a room with it (kept through sign-in).
- Result: "Défier un autre salon" (pick a room or share the link) and side-by-side podiums once both are closed.
