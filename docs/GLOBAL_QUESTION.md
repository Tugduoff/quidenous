# Global question of the day

One built-in deck question per day and language, the same in every room of that language, apart from the room's own question flow. Everyone answers it about someone of their room; after voting they see their room's tally and anonymous aggregates across rooms. Feature flag: `global_question` in `GET /api/features` (the app shows nothing without it; old apps never ask).

## The question

- **Day** = the UTC date (`date('now')` in SQLite), everywhere: picking, voting, "yesterday". It changes at 01:00 in Paris in winter, 02:00 in summer.
- **Language** = the room's language (`groups.language`): French rooms share one question, English rooms another.
- **Pool**: built-in deck questions (`server/deck/{fr,en}.txt`) of kind *pick* tagged `[famille]` or `[boulot]` (the vibes every room can take, so nothing awkward in a work or family room), never `[pimente]` (16+). About 100 per language.
- **Pick**: lazily, on the first request of the day, stored in `global_questions (day, language, question_id, text)` (insert-or-ignore: two first requests at once store one). Among the pool questions not asked in the last 90 days (when every one was, the ones asked longest ago), the index is a SHA-256 of `global:<day>:<language>`, so it is deterministic and the same for everyone. The text is copied, so a later deck edit doesn't change a past day.

## Who can answer

- A seat (`members` row) in a **room** (not a party; a party kept as a room counts as a room), not a party guest account, in a room with **at least 3 people** (`MIN_MEMBERS`): with two, "who of us" is a coin toss. Otherwise `GET /api/global` says `available: false` with `reason: party | guest | small_room` and the app hides the card.
- One vote per seat and day (`global_votes (day, language, group_id, voter_id, target_id)`, primary key `(day, language, voter_id)`), **final**. The target must be a current member of the voter's room, the voter included. Someone in two rooms answers once in each.
- Not part of the room's game: no round, no `votes` row, no push, not in history, stats or streaks. No rate limit beyond auth (a seat is required).

## Privacy

- **My room's tally**: counts per person (`target_id`, `count`), **never who voted** for whom, and only once I voted.
- **Aggregates** are computed over the rooms of my language with **at least 2 counted votes** today, and only shown once there are **at least 5 such rooms** (`MIN_ROOMS`) and I voted. Below that: the number of rooms so far ("2 salons sur 5 ont répondu"). Each fact needs 5 rooms of its own too (the "last winner" one only counts rooms that have a closed question).
- The payload never carries a name, a room id or a member id of another room, nor anything per room: only percentages and counts.
- Votes count while voter and target are still in the room and the room still exists (someone who leaves takes their vote and the votes for them along, as in the room's own results).

## Aggregates

Computed on the fly from the votes (no stored per-room result), over the counted rooms. Ties for first: every tied person counts as "the winner".

| id | meaning |
| --- | --- |
| `unanimous` | % of rooms where every vote went to the same person |
| `owner` | % of rooms where the winner is the room's owner |
| `newest` | % of rooms where the winner is the member who joined last (`members.created_at`; the owner never counts as the newcomer) |
| `last_winner` | % of rooms (among those with a closed *pick* question that got votes) where the winner also won that latest question |
| `self` | % of voters (in counted rooms) who picked themselves |
| `top_share` | average share of its room's votes the top person got |

Sorted by percentage, highest first; the app shows the first three (two for yesterday).

## API

- `GET /api/global` (X-Room = the room):
  - unavailable: `{ available: false, reason, min_members: 3 }`
  - else `{ available: true, day, language, question, my_vote: member id | null, room: { members, voters, tally: [{target_id, count}] | null }, global: { rooms, voters, min_rooms: 5, facts: [{id, pct}] | null }, yesterday: { day, question, tally, rooms, facts } | null }`. `yesterday` is yesterday's final numbers when there was a question (its tally shown whether or not I voted: it's over).
- `POST /api/global/vote { target }` (`target_id` accepted too) → the same view. `400 unknown_member` (not in my room), `409 already_voted`, `403 party | guest | small_room`, `409 no_question` (empty pool).

## App

- Vote tab: a one-line card under the header (Globe icon, the question, a check once answered), only when the server lists `global_question` and the room can answer. Tap: `/global`.
- `/global`: the question, my room's members as a grid of faces, "Voter pour X" (final). After: my room's bars (no voters' faces), "Tous les salons" with three fact rows (icon, sentence, thin bar) or the "pas encore assez de salons" state with the count, and "Hier" with yesterday's question, my room's winner and two facts.

Code: `server/src/global.rs`, `server/tests/global.rs`, migration `0046_global_question.sql`; `app/src/lib/global.ts`, `app/src/components/global.tsx`, `app/src/app/global.tsx`, `app/src/locales/{fr,en}/global.ts`.
