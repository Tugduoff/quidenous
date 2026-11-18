# Live reactions on results

Quick emoji reactions on a revealed result, daily rounds and party mode alike. Tapping one sends a little burst of that
emoji up the screen of everyone in the room who is looking at the result, live; the counts stay on the result afterwards.
Feature flag `result_reactions` in `GET /api/features` (an older server: the app shows nothing, and the party keeps its
old toggle row). Chat reactions on messages are something else (docs/ROOMS.md, "Chat reactions").

## Rules

- Palette, in this order: `😂 🔥 😱 👀 💀 ❤️ 👏 🙈` (`reactions::RESULT_EMOJIS`; the party's six plus 👏 🙈; a bare `❤` is
  `❤️`). Anything else is `400 bad_emoji`.
- Taps, not toggles: every tap counts. **30 taps per person per result** at most (`reactions::RESULT_CAP`), all emojis
  together. Taps over the cap are dropped quietly (the answer says how many were `accepted`); the app greys the bar out.
- Any member of the round's room, on a **closed** round (`409 not_reveal` while voting), at any time after (history
  included). A round of another room, or that doesn't exist: `404 not_found`.
- Blocks: the people I blocked are left out of what I see (counts and names), like chat reactions. In live events the
  app drops their taps itself (`user_id` is in every tap).
- Anonymous questions (docs/ANONYMOUS.md): reactions are not votes, they show who reacted as usual.
- Rate limit: shares the chat reactions' buckets (60 requests / minute, 600 / hour per account). The app batches taps,
  so a frantic thumb is a few requests.

## Storage

`round_reactions (round_id, member_id, emoji, count, created_at, updated_at)`, one row per person and emoji, `count` the
taps (`0050`, may need renumbering against other branches). The migration moves the old `party_reactions` rows in
(count 1) and empties that table (left in place so a rollback still starts). Account deletion removes the person's rows.

## API

`GET /api/rounds/{id}/reactions` and `POST /api/rounds/{id}/reactions` `{ "taps": [{ "emoji": "😂", "count": 3 }] }`
(1 to 8 entries, each count 1 to 30, otherwise `400 bad_taps`; the same emoji twice is fine, the counts add up). The taps
are counted in request order until the cap. Both answer:

```json
{
  "round_id": 12,
  "cap": 30,
  "mine": 4,
  "accepted": 3,
  "reactions": [{ "emoji": "😂", "count": 7, "mine": 3, "members": [{ "member_id": 5, "count": 4 }] }]
}
```

`mine` (top): my taps on this result, all emojis. `reactions`: palette order, only emojis with taps; `mine` there is my
count for that emoji; `members` oldest first. `accepted` only on `POST`. Party guests (accounts without email) can use
both: the seat is looked up in the round's room, not the current room.

Party: `POST /parties/{id}/react` (older apps) still toggles: if I have that emoji it is removed, else one tap is added
(within the cap; at the cap nothing changes). The party state's `reactions` reads the same table: `count` is the taps,
`mine` whether I have any, without the people I blocked.

## Live

`result_reacted` on the room socket, **only for sockets opened with `events=reactions`** in the query
(`/api/live?ticket=…&events=reactions`, the big screen `?screen=CODE&events=reactions`): older apps refetch everything on
an unknown event, so they never get it.

```json
{ "type": "result_reacted", "round_id": 12, "taps": [{ "member_id": 5, "user_id": 9, "emoji": "😂", "count": 3 }] }
```

`user_id` is null for a deleted account. Coalesced per round like `party_updated` (server/src/live.rs): the first one
goes out right away, the taps of the next 300 ms are merged (per person and emoji) into one trailing event. Party rooms
also get `party_updated`, for older apps.

The app adds the counts to what it shows and plays the bursts (its own taps are skipped: it showed them already). It
refetches `GET` when the result opens and when the socket reconnects, so a lost event only costs a burst.

## App

- `components/live-reactions.tsx`: the bar of 8 emojis under the result (daily `Result`, party reveal), counts under
  each, mine tinted. A tap: light haptic, a burst from that button, a count bump; taps are sent every 250 ms in one
  request. Long-press a count: who reacted (names and counts).
- The bursts are a fixed pool of Reanimated views animated on the UI thread (float up, drift, fade); new bursts reuse
  the oldest slot, so 40 phones mashing costs the same as one. More taps in one event than slots: the extras only count.
- Reduced motion: no bursts, the count changes with a fade.
- The party's big screen shows the bursts large, rising from the bottom edge, and the counts. It has no account, so no
  `GET`: the counts are the party state's `reactions` (refetched on `party_updated`), the bursts come from the socket.
- Pools: 24 slots under a phone's bar (at most 16 bursts per live event), 40 on the big screen. My own taps from another
  device are skipped live like the rest of mine (same `member_id`); they show at the next refetch.
- Code: `lib/result-taps.ts` (palette, cap, merging, the batcher; `result-taps.test.ts`), `lib/result-reactions.ts`
  (the query, sending, the live hand-off).
