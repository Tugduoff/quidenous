# Wrapped

« Ton année qui de nous » (personal, across my rooms) and « L'année de <salon> » (one room): a story of about ten full-screen cards about a calendar year, each shareable as a server-drawn card. Feature flag `wrapped` in `GET /api/features`. Code: `server/src/wrapped.rs` (room cards reuse `stats::compute_range`, the `/stats` computation over a date range); the app half: `app/src/lib/wrapped.ts`, `app/src/app/wrapped.tsx`.

## When

- `WRAPPED_FROM` / `WRAPPED_UNTIL` (`MM-DD`, default `12-01` / `01-31`): the window. When it spans New Year it shows the year it started in: from Dec 1 of Y to Jan 31 of Y+1, year Y. An unreadable value falls back to the default (warned at start-up).
- The reader's local date (`tz`) decides. Outside the window: `available: false` and no cards.
- **Preview**: admins (`ADMIN_USER_IDS`) may pass `preview=1` any time, and any `year` (2000 to next year). `WRAPPED_PREVIEW=1` (dev servers) makes it available to everyone, all year, as a preview. `preview: true` says it is shown thanks to a preview.
- `year` defaults to the window's year (outside it: the current year); another year is `400 bad_year` unless previewing.

## Privacy

- Only **finished** questions **opened** during the year (reader's time zone).
- Personal: only rooms (not parties, not deleted) where I have a seat **now**; only people with a seat now in one of those rooms (someone who left, a deleted account, a room I left never show up), by first name.
- Room: the room's current members only (as in the stats).
- Anonymous questions never say who voted for whom: no `crush`, `fan`, `flair`, `twins`, `duo` from them. They still count for votes received and who won (the result shows that much).

## Cards

Each card: `key`, a lucide `icon`, a `mascot` (`who` + `mood`), `title` (small caps), `big` (figure or name), `line` (one punchline, ≤ 110 characters, French or English, first names only, a few variants chosen deterministically like stats stories), `people` (0 to 2 faces), `question` (question cards), `shareable` (intro and outro are not). Cards without data are skipped; fewer than 3 data cards → `cards: []` (the app shows an empty state).

Personal, in order:

| key | what | shown when |
|---|---|---|
| `intro` | questions answered, rooms, days played | I voted at least once |
| `crush` | who I voted for most (not me) | 2 votes or more |
| `fan` | who voted for me most | 2 votes or more |
| `nemesis` | who won most often on pick questions where I got votes but didn't win | 2 times or more |
| `picked` | votes received (pick, ranking #1) and questions won | 1 vote or more |
| `best_question` | the pick question where I got the biggest share (3 votes or more) | |
| `flair` | % of my votes for someone else (pick, 3 votes or more) that went to a winner, and Tripot bets won | 5 such votes or more |
| `streak` | best day streak of the year, any room (docs/STREAKS.md) | 2 days or more |
| `title` | title of the year: highest tier earned during it (else the title I display); mascot = its family | |
| `room_question` | the year's hottest question across my rooms (most chat messages, 3 or more) | |
| `outro` | « On remet ça en <Y+1> ? » | always (not shareable) |

Room: `intro` (questions, votes, messages, members), `star` (most chosen), `crown` (most crowned), `crush` (the stats duo), `twins`, `predictor` (most chips won at the Tripot during the year; else `sheep`, the one most often on the winning side), `streak` (the member with the longest day streak of the year), `hottest`, `split` (most divisive), `outro`.

## API

```ts
type WrappedPerson = { name: string; color: string; photo_url: string | null; avatar: string | null };
type WrappedCard = {
  key: string; icon: string;
  mascot: { who: Mascot; mood: 'happy'|'accuse'|'sleepy'|'celebrate'|'sad'|'think'|'caught' } | null;
  title: string; big: string; line: string; people: WrappedPerson[]; question?: string; shareable: boolean;
};
type Wrapped = {
  year: number; scope: 'me' | 'room';
  available: boolean; preview: boolean; can_preview: boolean;
  from: string; until: string;   // 'YYYY-MM-DD' of the window for `year`
  room: string | null;           // scope=room: the room's name
  cards: WrappedCard[];
};
```

- `GET /api/wrapped?scope=me|room&year=YYYY&tz=<minutes east>&preview=1`. `scope=me` (default, signed in, no room needed); `scope=room` (room seat, `X-Room`; `403 not_in_room` without a seat there, `400 bad_scope` for a party or an unknown scope).
- `POST /api/wrapped/share { scope, year?, key, tz?, preview? }` → `{ token, path }`. Recomputes the Wrapped the caller sees and freezes that card into `stat_shares` (key `wrapped_<key>`, `label` « Wrapped 2026 » or « <salon> · 2026 », `member_ids` = the seats of the people shown, so erasing one of them removes it). A personal card belongs to my current room seat (the room I used last, else the most recently active). Same dedup (same card again = same link), 90-day expiry and revocation as stat stories. `404` when the card isn't in the current Wrapped or isn't shareable. On the picture, a question card shows its question instead of the punchline.
- `/s/{token}` and its `card.png`: the stat card in a Wrapped variant (label instead of the room name, pink tag, the year as a huge outlined watermark, a few confetti dots). `GET /shares` lists it with the label as its room. No mascot on the server card (the rig's moods and parts live in the app's renderer; redrawing them in Rust wasn't worth it).

## The December notice

The hourly badge sweep (`badges::spawn_sweeper` → `wrapped::notify_at`), while the UTC date is in the window (never in a mere preview): one inbox entry kind `wrapped` « Ton année qui de nous est prête » / "Your year in Qui de nous is ready" to every account that voted in the last 60 days, on its current room seat; once per account and year (`users.wrapped_notified`). No push.

## App

A story carousel (progress segments, tap, swipe, auto-advance unless reduced motion or held), one mascot per card, a share button on shareable cards, a "Moi / Ce salon" toggle; entry points on the Stats tab and the profile when `available` (or `can_preview`, labelled "Aperçu"), "Aperçu du Wrapped" on the admin screen, inbox kind `wrapped` opening it. Everything only when the server lists `wrapped`.
