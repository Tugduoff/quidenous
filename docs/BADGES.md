# Titles & badges

A catalogue of badges earned from real play, three tiers each (bronze, argent / silver, or / gold). Each earned badge is a *title* (its name); someone picks one to show next to their name. Feature flag `badges` in `GET /api/features`. Code: `server/src/badges.rs`; the app half: `app/src/lib/badges.ts`, `app/src/app/badges.tsx`, `app/src/components/title-chip.tsx`.

## Rules

Per **account**, counted over all its seats, current or former, in rooms of kind `room` (deleted rooms included: it is their own history), never in parties except for `fetard`. Badges are never taken back.

| key | FR | EN | metric | tiers | icon | mascot |
|---|---|---|---|---|---|---|
| `pilier` | Le pilier | The pillar | votes cast (any kind, any question) | 50 / 250 / 1000 | landmark | pave |
| `suspect` | Le suspect habituel | The usual suspect | finished questions won (pick: most votes, ties count; ranking: most points) | 5 / 25 / 100 | fingerprint | bulle |
| `evidence` | L'évidence | The obvious one | finished pick questions with 3 votes or more, all of them for me | 1 / 5 / 20 | target | bulle |
| `devin` | Le devin | The oracle | Tripot bets won (payout > 0, not refunded) | 3 / 15 / 50 | eye | pave |
| `flair` | Le flair | The nose | my votes for someone else on finished pick questions (3 votes or more) that went to a winner | 25 / 100 / 400 | radar | zeste |
| `incorruptible` | L'incorruptible | The incorruptible | pick and ranking votes cast, held only while I have **never** voted for myself | 30 / 150 / 500 | shield-check | pave |
| `narcisse` | Narcisse | Narcissus | votes for myself (pick, or myself first in a ranking) | 5 / 20 / 50 | flower-2 | paillette |
| `marathon` | Le marathonien | The marathoner | best day streak in any room (docs/STREAKS.md rules, my time zone) | 7 / 30 / 100 | flame | paillette |
| `gachette` | La gâchette | Quick draw | finished questions where I voted first (2 voters or more) | 10 / 50 / 200 | zap | zeste |
| `plume` | La plume | The quill | questions I wrote (`origin = 'written'`) that were asked (closed with votes) | 3 / 15 / 50 | feather | pave |
| `bavard` | Le bavard | The chatterbox | chat messages sent | 50 / 250 / 1000 | message-circle | paillette |
| `relou` | Le relou | The nag | nudges sent | 10 / 50 / 200 | bell-ring | zeste |
| `couche_tard` | Le couche-tard | The night owl | votes cast between 23:00 and 04:59 in my time zone (`notification_prefs.tz_offset`, else UTC) | 10 / 50 / 200 | moon | roupi |
| `fetard` | Le fêtard | The party animal | parties (`groups.kind = 'party'`) where I voted at least once | 1 / 5 / 20 | party-popper | paillette |

- Choose questions only count for `pilier`, `gachette` and `couche_tard` (their votes row points at the voter: no winner, no self vote).
- **Anonymous questions** (docs/ANONYMOUS.md): their votes count for `pilier`, `gachette` and `couche_tard`; nothing about *who was picked* on them counts (`suspect`, `evidence`, `flair`, `narcisse`), and the `incorruptible` self-vote check and count only look at the other questions.
- Hints say how to reach the next tier with its number (« Vote 250 fois » / "Vote 250 times"), or, at gold, how it was earned (« 1000 votes et plus »).

## Awarding

`badges::evaluate(state, user_ids, group_id)` computes the metrics (four aggregate queries per account over its seats, indexed by `votes_voter`, `messages_member`, `nudges_from`, `tripot_bets_member` and rounds per room, plus the streak replay of `streaks.rs`), then `award` in one write transaction:

- every tier reached is a row `INSERT OR IGNORE`d into `user_badges`: evaluating twice inserts nothing, and two evaluations racing each other can't both report the same tier;
- one inbox entry per badge for its **highest new tier** (kind `badge`, `round_id` null): « Nouveau titre : Le pilier (or) » / "New title: The pillar (gold)", plus the usual `InboxChanged` live event. No push. It goes to my seat in the room where it happened when I'm still there, else the room I used last (`users.last_group_id`), else my most recently active room; no room seat (a party guest) = no entry, the badge is still earned;
- the **first evaluation of an account** (`users.badges_at` null) backfills silently: no inbox flood on deploy;
- when nothing is new and the account was evaluated before, nothing is written at all (spares the SD card), so `badges_at` is the last evaluation that changed something.

When it runs:

- **After a question closes** (`routes::notify_round`, every path that closes a question with votes: last vote, skip, removal, the 24 h quorum; and a party question closed by its last vote), in a spawned task after the commit; errors are logged, never the request's. It evaluates the question's voters, the people voted for or ranked, and its author. The day streaks are replayed for that room only (the others haven't changed).
- **`GET /badges`** evaluates the reader first, so a fresh threshold shows at once.
- **Hourly sweep** (`badges::spawn_sweeper`, first run at start-up): accounts whose seats were used in the last 2 hours (`members.active_at`), every room. It catches what doesn't close a question in the room: Tripot bets settled, nudges, chat, party questions closed at their deadline. The same sweep posts the Wrapped notice (docs/WRAPPED.md).

## Storage

Migration `0052_badges_wrapped.sql`:

- `user_badges(user_id → users ON DELETE CASCADE, key, tier 1..3, group_id → groups (room where it was crossed, null from the sweep or the collection), earned_at, PRIMARY KEY (user_id, key, tier))`.
- `users.title` (the displayed key), `users.badges_at` (null = never evaluated), `users.wrapped_notified` (year).
- Inbox `kind` CHECK rebuilt (as in 0038) with `badge` and `wrapped`; indexes `votes(voter_id)`, `nudges(from_id)`, `tripot_bets(member_id)`.

Account erasure (`accounts/delete.rs`) deletes the `user_badges` rows and clears `title`, `badges_at` and `wrapped_notified`. Badges are derived from data the app already keeps (votes, messages, nudges, bets); the title is the user's own choice among them.

## API

```ts
type Mascot = 'zeste' | 'bulle' | 'pave' | 'paillette' | 'roupi';
type Badge = {
  key: string; icon: string; mascot: Mascot;
  name: string;              // the title, reader's language
  hint: string;              // how to reach the next tier, or how it was earned when gold
  tier: 0 | 1 | 2 | 3;       // highest earned (0 = locked)
  tiers: number[];           // thresholds, length 3
  progress: number;          // current metric value
  earned_at: string | null;  // of the highest earned tier
};
type TitleChip = { key: string; name: string; icon: string; tier: 1 | 2 | 3 };
```

- `GET /api/badges` (signed in): `{ title: string | null, earned: number, total: number, badges: Badge[] }`, catalogue order.
- `PUT /api/me/title { key: string | null }` → `{ title }`. `400 bad_title` (unknown key or not a string), `403 not_earned`; `null` (or no key) clears.
- `GET /api/titles` (room seat, `X-Room`): `{ members: { "<member id>": TitleChip } }` for the current members of my room who display a title they hold. One query. A party answers `{ members: {} }`.
- `GET /api/members/{id}/badges` (room seat): `{ title: TitleChip | null, badges: [{ key, name, icon, mascot, tier }] }`, highest tier per key, catalogue order. `404` unless that seat is a current member of my room.

Names, hints and inbox texts are in the reader's language (`users.locale`).

## App

A "Titres et badges" screen (medals: earned = tier-coloured disc with the icon, locked = silhouette, hint and progress; a sheet with the family's mascot and "Afficher ce titre"), a small title chip next to names (members, results, profile, stats profile sheet) from one `/titles` query per room, inbox kind `badge` opening the screen. Everything only when the server lists `badges`.
