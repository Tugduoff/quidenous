# Party mode (soirée)

Status: server built (`server/src/party.rs`, migration `0026_party.sql`, tests `server/tests/party.rs`), 2026-11-13. A mode for evenings with people in the same place: one question at a time for everyone, a countdown, a big reveal, then the next one when the owner taps "Suivante" or 75 % of the players did.

## Model

A party is a room (`groups` row) with `kind = 'party'`, so rounds, votes, members, history and the round view work unchanged, and "keep playing as a normal room" is just flipping `kind` back to `'room'`.

- `groups.kind TEXT NOT NULL DEFAULT 'room'` (`'room' | 'party'`).
- `groups.party_ends_at` (RFC 3339 UTC): after it the party is **over**: a read-only memory (results stay, no new questions, votes / next / start answer `410 party_over`). Any question still open at that moment is closed.
- `groups.party_timer INTEGER NOT NULL DEFAULT 30`: seconds per question, owner setting, one of `15, 20, 30, 45, 60`.
- `rounds.deadline_at` (party rounds only): opened_at + timer.
- `party_players (round_id, member_id)`: who was in the party when the question opened. Only they vote on it and only they can be picked; someone joining mid-question votes from the next one (`409 joined_late`).
- `party_taps (round_id, member_id)`: "Suivante" taps on a revealed question.
- `party_reactions (round_id, member_id, emoji)`: quick reactions on a result, toggled. Allowed: `😂 🔥 😱 👀 💀 ❤️`.
- `users.guest INTEGER NOT NULL DEFAULT 0`: account created by a party join without an account (token only, like a legacy account). It can later add an e-mail + password or link Google/Discord (`/api/auth/credentials`, OAuth link) and keeps its seats and history; `guest` reads false once the account is secured.

## Flow

1. **Lobby**: a new party is *paused* (`groups.paused_at` set at creation) so nothing opens by itself. The owner taps "Commencer" (`POST /parties/{id}/start`, needs 2 players, `409 need_two`).
2. **Voting**: one pick question (parties draw `kind = 'pick'` questions only: the room's own, then the built-in deck of its language and vibe). `deadline_at` = now + timer. The round closes when every eligible player still in the party voted, or at the deadline (with or without votes). No push, no inbox for party questions or results.
3. **Reveal**: a closed party round always pauses the party (no next question by itself). Results are shown together.
4. **Next**: the owner's tap opens the next question right away. Anyone else's tap is counted; when taps reach `needed = ceil(0.75 × players)` (players = active members now), the next question opens. Tapping twice is a no-op; `DELETE` un-taps.
5. **Over**: at `party_ends_at` (or the owner's "Terminer") the party is a memory. The owner can turn it into a normal room (`POST /parties/{id}/keep`): `kind = 'room'`, end and timer cleared, pacing back to the room defaults, the next question opens like any room.

Timers: when a party round opens, the server spawns a task that sleeps until `deadline_at` and closes it if still open (idempotent `UPDATE … WHERE closed_at IS NULL`). A sweeper every 5 s (first run at startup) closes overdue party rounds and ends parties past `party_ends_at`, so a restart loses nothing.

Leaving (`POST /rooms/{id}/leave`, or the owner removing someone) re-checks the open round (everyone left voted → close) and the taps (75 % of fewer players may now be reached). Closing the app is not leaving: the timer covers absent people.

Parties don't count for the "people I share a room with are friends" rule, send no push/inbox for rounds, and are skipped by stats if mixing them in would skew a room's stats (they are separate rooms anyway).

## Join without an account

`POST /api/parties/join { code, name?, color? }`:

- With a valid `Authorization: Bearer` token: that account takes a seat (or gets its seat back).
- Without: `name` (1-24 chars, cleaned like other names) and `color` (`#RRGGBB`, from the app palette) are required; a guest user + token is created (`users.guest = 1`). Not blocked by `AUTH_REQUIRED` (that's the point), but rate-limited: 30 guest joins / hour / IP (a whole party shares one Wi-Fi), and at most 40 players per party (`409 party_full`).
- A name already taken by another player of that party (case-insensitive, people still in it): `409 name_taken`. Guests only: an account joins under its own name.
- Missing / bad `name` or `color` for a guest: `400 bad_name` / `400 bad_color`. An account may send `color` too (else its account colour).
- An invalid token: `401` (the app can retry without it).
- Unknown code or deleted party: `404 not_found`; an over party: `410 party_over`.
- Answer: `{ party_id, member_id, token? }` (`token` only when a guest was created). The app stores it like any session and sets the room to `party_id`.

Party codes: 5 characters from `ABCDEFGHJKLMNPQRSTUVWXYZ23456789`, unique among live rooms, stored in `groups.join_code` (the room invite code column). `POST /rooms/join` with a party code works too (signed-in people) and returns `kind` (same `410 party_over` / `409 party_full`). The old anonymous `POST /join` refuses party codes (`400 bad_code`).

## API

All under `/api`, JSON, errors `{"error": code}`. `{id}` = the party's room id; the caller must have a seat (`403 not_in_room`).

| Request | Who | Body | Answer |
| --- | --- | --- | --- |
| `POST /parties` | account (any token) | `name?` (0-40), `vibe?`, `language?`, `hours` (`2, 4, 8, 12`, default 4), `timer?` (default 30) | `{ id, code }` |
| `POST /parties/join` | anyone | see above | `{ party_id, member_id, token? }` |
| `GET /parties/{id}` | player | | party state (below) |
| `POST /parties/{id}/start` | owner | | state · `409 need_two`, `409 started` |
| `POST /parties/{id}/vote` | eligible player | `target_id` | state · `409 joined_late`, `409 already_voted`, `409 no_open_round`, `400 unknown_member` |
| `POST /parties/{id}/next` | player | | state · owner: opens next; others: tap · `409 not_reveal` |
| `DELETE /parties/{id}/next` | player | | state |
| `POST /parties/{id}/react` | player | `round_id, emoji` | state · `400 bad_emoji` |
| `PATCH /parties/{id}` | owner | `name?, timer?` | state · `400 bad_timer` |
| `POST /parties/{id}/end` | owner | | state (over now) |
| `POST /parties/{id}/keep` | owner | | `{ id }` (now a normal room) |
| `GET /parties/screen/{code}` | anyone with the code | | party state without `me` (big-screen view) |

Leaving uses `POST /rooms/{id}/leave`. The memory (list of questions and winners) uses the room endpoints with `X-Room: {id}` (`GET /history`, `GET /rounds/{rid}`).

### Party state

```json
{
  "id": 12, "kind": "party", "name": "Chez Louis", "code": "K7PQM", "vibe": "potes", "language": "fr",
  "timer": 30, "ends_at": "2026-11-14T02:00:00.000Z", "now": "2026-11-13T22:41:07.512Z",
  "phase": "lobby | voting | reveal | over",
  "players": [{ "id": 5, "name": "Tug", "color": "#FF7A45", "photo_url": null, "avatar": null, "role": "owner", "guest": false }],
  "me": { "member_id": 5, "role": "owner", "guest": false },
  "round": null,
  "played": 7
}
```

`round` (null in the lobby, the last closed one when over):

```json
{
  "id": 44, "number": 3, "question": "Qui finit la soirée en dansant sur la table ?",
  "opened_at": "…", "deadline_at": "…", "closed_at": null,
  "eligible": [5, 6, 7], "voted": [5, 6], "my_vote": 6,
  "results": [{ "member_id": 6, "votes": 2, "voters": [5, 7] }],
  "next": { "taps": [5], "needed": 3, "players": 4, "mine": true },
  "reactions": [{ "emoji": "😂", "count": 2, "mine": true }]
}
```

`results`, `next`, `reactions` are null while voting (votes are revealed together). `voted` lists who voted (not for whom). `my_vote` is null without `me`. `now` lets the app correct its clock for the countdown.

### Live

The existing socket (`/api/live?token=`) carries the party's room events: `member_joined`, `vote_cast`, `round_closed`, `round_opened`, `member_removed`, `room_updated` (settings, start, end, keep) and a new `party_updated { round_id }` (taps, reactions). The app refetches the party state on any of them. The big screen subscribes without an account: `/api/live?screen=<code>` (party rooms only, same events).

`GET /api/features` lists `party`.

### Details (as built)

- `POST /parties`: `name` empty or missing = "Soirée de {host}" / "{host}'s party" (host's language); `language` missing = the host's account language; `400 bad_hours`, `400 bad_timer`, `400 bad_vibe`, `400 bad_language`, `400 bad_room_name` (> 40).
- `eligible` lists the players of that question still in the party (they are the ones the close waits for); `voted` lists everyone who voted on it.
- `reactions` lists only emojis with at least one reaction, in the order `😂 🔥 😱 👀 💀 ❤️`. Reacting on an open question: `409 not_reveal`.
- Over: `vote`, `next`, `start`, `react`, `PATCH` answer `410 party_over`; `end` again changes nothing; `next` is null in the state. `keep` works at any time (owner).
- After `keep`, `GET /parties/{id}` still answers (with `kind: "room"`, so the app can go to the room); the other party endpoints answer `409 not_party`.
- Party rooms refuse room pacing (`PATCH /rooms/{id}` with `owner_next` / `next_delay`: `400 bad_setting`; `POST /rooms/{id}/next`: `409 party`) and nudges (`409 party`).
- `GET /rooms` and `GET /group` carry `kind` (`room` | `party`); parties add `ends_at` and `over`.
- `groups.party_ended_at` (internal) records that the server processed the end (open question closed, `room_updated` sent), so the sweeper does it once.
- Deadline closes and the sweeper go through `party::sweep_at(state, now)` (explicit time, used by the tests); the per-question task and the 5 s sweeper call it with the database clock.

### Deleted accounts in member payloads

Member objects (`/group` members and `former`, `/me`, join answers, party `players`) carry `deleted: true` for the seat of a deleted account (absent otherwise); `name` keeps the stored placeholder "Compte supprimé" for old apps, newer apps show their own label. Texts the server writes for a reader (CSV export, share cards and pages, stats stories) use the reader's language ("Deleted account" in English).

## App

- Rooms screen: "Soirée" button → create (name optional, vibe, duration, timer) → lobby.
- `/party/[id]`: lobby (big QR, code, share link, players, owner "Commencer"), question with countdown, reveal, "Suivante" with the filling count, over (memory, owner "Continuer en salon", guest "Créer un compte").
- `/party/join/[code]`: join link target (`https://<host>/party/join/K7PQM`), works without an account (name + colour).
- `/party/screen/[code]`: big-screen view (web, no account): QR + code, question, live vote count, timer, big reveal. Not behind the phone-only gate.
- Opening a party room from the room switcher goes to `/party/[id]`.
