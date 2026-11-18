# Aujourd'hui

The « Aujourd'hui » home: every room I'm in, at a glance, the one that waits for me first. One card per room (its question to answer, a result to see, a bet to place, the votes still missing), so someone in several rooms doesn't have to open each one. Feature flag `today` in `GET /api/features`. Code: `server/src/today.rs`, app `app/src/lib/today.ts`.

Plus the first-run intro, once per account ([Intro](#intro) below), flag `intro`.

## API

`GET /api/today` (any signed-in account, no `X-Room`: it is about all of them) →

```
{ rooms: [{
    id, name, role, members, muted,
    state,                 // vote | result | tripot | wait | idle
    round,                 // the open question: { id, number, text, kind, anonymous, opened_at } | null
    voted,                 // I voted on it
    votes, needed,         // votes in, votes needed to close it (0, 0 without an open question)
    result, unseen,        // the newest result I voted on and haven't seen { id, number, text, kind, closed_at } | null, and how many
    tripot,                // { can_bet, balance } | null when Tripot is off in the room
    streak,                // { days, today } | null (no streak: 0 days and no vote today)
    unread,                // unread chat in my inbox for this room (messages and mentions)
    next_open_at,          // no open question: when the next one opens by itself (pacing), else null
  }] }
```

Every room I sit in: parties, deleted rooms and seats I left or was removed from are left out. An account without a room (a party guest, someone who left them all) gets `{rooms: []}`. Nothing else than my own seats is read: another member's Today never shows a room of mine they aren't in.

The fields follow the rules of the screens they summarise, so a card never disagrees with the room once opened:

- `votes` / `needed`: as `GET /round/current` (`game::view`): everyone still in the room, minus the people the room agreed to skip on this question (a majority of `skips`, `game::skipped`) and the people who are away (`game::away`, the last 3 questions closed without them). The away rule is one SQL fragment, `game::away_sql!`, shared by both.
- `result` / `unseen`: as `GET /round/unseen` (`game::unseen_ids`): closed questions I voted on, numbered past my `seen_round`. Marking the newest seen (`POST /rounds/{id}/seen`) clears them all.
- `tripot`: as `tripot::view` (`can_bet`: a pick or ranking question, not anonymous, open, no bet of mine yet, at least 1 chip; `balance`: my chips this month).
- `streak`: my day streak in the room (docs/STREAKS.md), `today` once I voted today in my time zone.
- `unread`: inbox rows of kind `message` / `mention` not read yet, without the people I blocked (same filter as `GET /inbox`, `inbox::not_blocked!`).
- `role`, `muted`: my seat's.

## States and order

What a room asks of me, most pressing first:

1. `vote`: an open question I haven't answered and can: not skipped by the room on it, and the room has at least 2 people (a question left open in a room down to one person takes no vote, `game::vote`'s `alone`).
2. `result`: a result I voted on and haven't seen (an open question I already answered, or none).
3. `tripot`: I voted, and can still bet on the open question.
4. `wait`: I voted, the others haven't all yet.
5. `idle`: nothing for me: no open question (paused by pacing, waiting for its owner, out of questions; `next_open_at` says when it comes back when it does by itself), or one I can't vote on (skipped, alone).

`rooms` is sorted by state, then the room I used last (`users.last_group_id`) first, then the latest activity (the open question's `opened_at` or the unseen result's `closed_at`), then the newest room.

## Queries

A fixed number of queries whatever the number of rooms, no per-room loop (no N+1): one for my seats (with each room's member count, Tripot switch, pacing, my unread count and my chip balance as subqueries), then five run together (`tokio::try_join!`) on that list of seats, passed as one JSON array (`json_each`):

- the open questions, with their vote count, my vote and my bet;
- the skips on them (counted per target; the majority is applied in Rust with each room's size);
- the away members of the rooms with an open question;
- the unseen results (a window function keeps the newest per room and counts them);
- the streaks: `streaks::Room::load_seats` loads the rounds, seats, and only *my* seats' votes and skips of all those rooms in four queries, then builds each `streaks::Room` as `Room::load` would (`Room::assemble`), enough for my own streak (not the room's or anyone else's).

## Live: `others=1`

The live socket follows one room. With `GET /api/live?…&others=1` (an account, not a party screen), it also sends a compact frame when something changes in another of my rooms:

```
{ "type": "room_activity", "room_id": 7 }
```

No content: the app refetches `/today` (and the room list), which applies the usual rules. Only events that can change a card count (votes, questions opened or closed, skips, chat posted or deleted, bets, room settings, people joining or leaving, a room deleted, inbox changes). Throttled per room at **one per second** (`today::ACTIVITY_GAP`): the first goes out at once, the ones within the next second become a single trailing event at the end of the gap, so a burst of votes is one refetch and the last change is never lost. The list of my rooms is loaded when the socket opens and again at each membership check (a room joined since is followed, one left is no longer). Without `others=1` nothing changes for older apps. Code: `today::OtherRooms`, wired in `routes::live` / `relay`.

## App

A tab « Aujourd'hui » (`app/(tabs)/today.tsx`), shown when the server lists `today` and I'm in at least two rooms (parties aside): with one room it would be a one-card detour before the vote. The answer is remembered for the next start so the tab doesn't pop in or out; a plain cold start (icon, bare web address, not a notification or a link) lands on it. A tap on a card opens the room. Older servers (no `today` flag): no tab, and the socket is opened without `others=1`.

## Intro

A first-run intro with la bande (the mascots, docs/MASCOTS.md): a demo round the Petons play, shown once per account, on whichever device signs in first.

- `GET /api/intro` → `{show}`: true while the account never finished nor skipped it, and isn't a party guest (`auth::guest_sql!`: a guest came to play one party, not to learn the app).
- `POST /api/intro` `{skipped?}` → `{ok}`: done, finished or skipped (`skipped` is accepted, not stored: no use for it yet). Idempotent: the first time is kept. A body is optional.
- Storage: `users.intro_done_at` (migration 0053), NULL until done. The migration marks every account that already exists as done: people who use the app today never see it, only new accounts do.
- Feature flag `intro`. A server without it: no intro, for good.

App side (`app/src/lib/intro.ts`, `intro-sync.ts`): the device remembers the answer for the session in `qdn.introDone` (cleared at sign-in and sign-out): `no` (the server said no: older account, guest, older server) is never asked again; `done` / `skipped` means finished here and the server not told yet (offline), so it isn't shown again and the `POST` is retried at the next start. With nothing remembered, the app asks `/intro` alongside `/me` and gives up after 2.5 s (no intro this time, asked again next start): an existing account never waits on it. `?intro=1` on the web shows it again for screenshots and QA (signed in; finishing it tells the server nothing).

Migration number: 0053 (0049 widget, 0050 reactions, 0051 said, 0052 badges & Wrapped).

## Tests

`server/tests/today.rs` (states, order, exclusions, unread, streak, the intro and its backfill, `room_activity` on a real socket) and the throttle's unit test in `today.rs`.
