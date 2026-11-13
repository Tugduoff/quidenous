# Stats

One endpoint feeds the Stats tab: `GET /api/stats?period=7d|30d|all&tz=<minutes east of UTC>`.
Everything is computed in Rust from finished questions only (an open question would leak who voted for whom).
Without parameters (older apps) it answers for all time, UTC, and keeps every field older apps read.

- `period`: `7d` and `30d` keep questions opened since local midnight 6 / 29 days ago (today included, so the
  window is exactly the `activity` buckets); `all` (default) keeps everything. Anything else: `400 bad_period`.
  Nudges and chat messages count through their (finished) question: a message under a question of the period counts,
  a nudge on the open question counts once it is finished. `author` counts questions written in the period.
- `tz`: the phone's offset (`-new Date().getTimezoneOffset()`), used for the hour heatmap, night owls / early birds
  and day buckets. Default 0, clamped to ±14 h, unreadable = 0.
- Awards, stories and profiles only name current members; removed members' votes still count in totals.
- An award is `null` when the data doesn't support it (too few votes, too few members): the app hides it, never
  shows an empty section.
- `GET /features` lists `stats_v2` when the server has all this.

## Response

```ts
type Award = { member_id: number; value: number; total?: number };          // total: denominator, ratio awards only
type Duo = { voter_id: number; target_id: number; value: number; total?: number };
type Pair = { a_id: number; b_id: number; value: number; total: number };   // a_id < b_id
type RankAward = { member_id: number; avg: number; count: number };          // avg position, 1 decimal
type QuestionRef = { round_id: number; number: number; question: string; kind: 'pick' | 'rank'; value: number; total: number };
type Story = {
  key: string;              // award key it comes from (see table), unique in the list
  icon: string;             // lucide icon name, kebab-case, from the table below
  title: string;            // the award title, in the reader's language
  big: string;              // the big figure: "14", "6 d'affilée" / "6 in a row", "3 h 12"
  line: string;             // one punchline, French or English (the reader's `users.locale`), <= 90 chars, names already in it
  member_ids: number[];     // 0, 1 or 2 members shown with their photo
  round_id?: number;        // question stories
};
type MemberStats = {
  member_id: number;
  present: number;          // finished questions of the period opened after they joined (or that they voted on)
  votes_cast: number;
  received: number;         // votes received (pick #1s and ranking #1s)
  wins: number;             // questions won, ties count for everyone tied (ranking: most points)
  win_rate: number;         // wins / present, 0..1 (0 when present is 0)
  messages: number;
  self_votes: number;
  median_vote_seconds: number | null;   // >= 3 votes
  avg_rank: number | null;              // ranking questions, >= 2 of them
  top_voters: { member_id: number; value: number }[];   // who votes for them most, <= 3, not themselves
  top_targets: { member_id: number; value: number }[];  // who they vote for most, <= 3, not themselves
  best: QuestionRef | null;   // pick question where they got the biggest share (value = their votes)
  worst: QuestionRef | null;  // ranking question where their average position was worst (value = avg position rounded, total = people ranked)
  titles: string[];           // award keys they hold this period (pair awards and never_chosen included)
};
type Stats = {
  period: '7d' | '30d' | 'all';
  members: number;            // current members
  // Kept from v1 (older apps read them)
  questions: number; votes: number; unanimous: number; ties: number; nudges: number;
  seconds_voting: number; seconds_waiting_last: number; median_round_seconds: number | null;
  most_chosen: Award | null; most_crowned: Award | null; narcissist: Award | null;
  fastest: Award | null; slowest: Award | null; last_voter: Award | null;
  nudger: Award | null; most_nudged: Award | null; author: Award | null;
  duo: Duo | null; never_chosen: number[];
  // New
  messages: number;
  contrarian: Award | null; sheep: Award | null; loyal: Duo | null; twins: Pair | null;
  kingmaker: Award | null; streak: Award | null; longest_streak: Award | null; comeback: Award | null;
  night_owl: Award | null; early_bird: Award | null; chatterbox: Award | null; silent: Award | null;
  best_rank: RankAward | null; worst_rank: RankAward | null;
  most_split: QuestionRef | null; most_unanimous: QuestionRef | null; hottest: QuestionRef | null;
  stories: Story[];                 // 0..6, funniest first
  edges: { voter_id: number; target_id: number; value: number }[];   // who votes for whom, self included, non-zero only, current members on both ends
  heatmap: number[][];              // [7][24] votes cast, weekday 0 = Monday, local hours
  activity: { bucket: 'day' | 'week'; start: string; questions: number[]; votes: number[] };  // start = YYYY-MM-DD local
  profiles: MemberStats[];          // every current member, by id
};
```

`activity`: 7d / 30d give exactly 7 / 30 daily buckets ending today (local). `all` starts at the first question's
day, daily up to 90 days, weekly beyond (start = the Monday of the first week). Questions count on the day they
opened, votes on the day they were cast. No question at all: empty arrays.

`QuestionRef` values: `most_split` value = people chosen, total = votes; `most_unanimous` value = total = votes;
`hottest` value = messages, total = people who wrote; profile `best` value = their votes, total = votes.

Average position (`avg_rank`, `best_rank`, `worst_rank`): per ranking question, the average position voters gave
them; then the mean over questions.

## Awards

| key | title | icon | rule (ties: higher value, then lower member id) |
|---|---|---|---|
| most_chosen | La star | star | most votes received |
| most_crowned | Collection de couronnes | crown | most questions won |
| narcissist | Narcisse | flower-2 | most votes for themselves |
| fastest / slowest | Gâchette facile / Prend son temps | zap / snail | median vote delay, >= 3 votes, >= 2 people qualify |
| last_voter | Tout le monde attend | hourglass | most times the last vote |
| nudger / most_nudged | Relou officiel / Toujours relancé | bell-ring / alarm-clock | nudges sent / received |
| author | La plume | feather | questions written |
| duo | Le crush | heart | voter → target pair with the most votes (> 1), not self |
| never_chosen | Les invisibles | ghost | members with zero votes received (when there are votes) |
| contrarian | Le contrarian | shuffle | biggest share of votes for a non-winner; pick questions with >= 3 votes; >= 3 such votes (value = against, total = eligible); >= 3 members and >= 2 people qualifying |
| sheep | Le mouton | users | biggest share of votes for a winner, same pool; not the contrarian |
| loyal | Le fidèle | anchor | biggest share of one's (non-self) votes going to the same person, >= 4 non-self votes and >= 3 for that person; value = for that person, total = non-self votes cast |
| twins | Les jumeaux de vote | copy | pair of voters picking the same person most often (pick and ranking #1), >= 3 shared questions, >= 2 same picks, >= 3 members; value = same pick, total = shared |
| kingmaker | Le faiseur de rois | gavel | votes for a sole winner who won by exactly one vote (not their own win), pick questions with >= 3 votes; >= 2 |
| streak | Série en cours | flame | consecutive questions won up to the latest one, >= 2 |
| longest_streak | Record de série | trophy | longest run of consecutive wins in the period, >= 3 |
| comeback | Le revenant | rotate-ccw | won after the longest drought (questions since previous win, or since joining), >= 5 |
| night_owl | Couche-tard | moon | most votes cast 23:00–04:59 local, >= 2 |
| early_bird | Lève-tôt | sunrise | most votes cast 05:00–08:59 local, >= 2 |
| chatterbox | Le bavard | message-circle | most chat messages, >= 1 |
| silent | Le roi du silence | volume-x | fewest messages among members who voted in the period; >= 10 messages in total, >= 3 members; not the chatterbox |
| best_rank / worst_rank | Toujours sur le podium / Lanterne rouge | medal / turtle | best / worst average position on ranking questions, >= 2 each; different people |
| most_split | La plus clivante | split | pick question with the most distinct people chosen (>= 4 votes, >= 3 distinct); tie: fewer votes for the winner |
| most_unanimous | La plus unanime | handshake | unanimous question with the most votes (>= 3) |
| hottest | Le moment le plus chaud | flame | question with the most chat messages (>= 3) |

Question awards break remaining ties on the most recent question. Ranking questions are won on points (with N
ranked, 1st gets N points); a ranking question nobody ranked on (old apps only) falls back to the #1 votes.

## Stories

The server picks up to 6 stories from the awards above, funniest first (streak >= 3, comeback, narcissist, crush,
twins, contrarian, kingmaker, night owl, slowest, hottest, most_split, then the rest), skipping awards that are null
and never using the same member in more than 2 stories. Copy is French or English, the reader's language (`users.locale`; a shared story keeps the sharer's), short, cheeky, never mean. Rooms with the
`pimente` vibe (16+) get spicier lines for some stories.

The rest, in order: loyal (unless it is the same pair as the crush), longest_streak (unless it is the current streak),
most_chosen, most_crowned, sheep, chatterbox, silent, early_bird, fastest, last_voter, most_unanimous, best_rank,
nudger, most_nudged, author, worst_rank, then a streak of 2. Thresholds on top of the award's: narcissist >= 2.
`never_chosen` is never a story. Lines use first names only and pick a variant deterministically from the figures,
so they don't read like one template but don't change between two loads.

Sharing: `POST /api/stats/share { period, key, tz }` stores a snapshot of that story and returns `{ token, path }`;
`/s/{token}` and `/s/{token}/card.png` serve it like a shared question (same 1200×630 card style, first names and
colours only). `period` defaults to `all`, `tz` to 0; `400 bad_period`, `404` when the stats have no story with
that key. Each call makes a new link (the snapshot is frozen: table `stat_shares`).
