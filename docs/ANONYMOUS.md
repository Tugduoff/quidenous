# Anonymous votes

A question whose result shows counts and percentages only, never who voted for whom: for the questions nobody would answer with their name on it. Feature flag `anonymous_votes` in `GET /api/features`.

## Rules

- **Room rule** (owner, Paramètres du salon > Votes): `never`, `per_question` (default: each question says) or `always`. `PATCH /api/rooms/{id} {anonymous_votes}` (`400 bad_setting` for another value, or on a party); `GET /api/group` → `settings.anonymous_votes`.
- **Per question**: `questions.anonymous`, set from the composer's switch (shown only under `per_question`), `POST /api/questions {anonymous: true}`, the `[anonyme]` / `[anonymous]` tag in an imported file or a pack file (combines with `[classement]` in any order, docs/PACK_FILES.md), or a pack's question once installed. `GET /api/questions/queue` → `custom[].anonymous`.
- **Decided when a question opens, kept on the round** (`rounds.anonymous`): `always` → yes, `never` → no, `per_question` → the question's flag. A later rule change never reveals a past or open round. It can only make the *open* round anonymous (switching to `always`, or to `per_question` while the open question asks for it): nobody has seen a result yet, so that is safe. Nothing ever goes from anonymous back to named.
- **Pick and ranking questions only.** A choose question's result *is* who took what and who guessed whom (docs/CHOOSE.md), a said question's who wrote what (docs/SAID.md): the flag is dropped on them and the room rule doesn't apply. **Parties** are never anonymous: their reveal is played on a shared screen, in the same room (docs/PARTY.md); a party kept as a room follows the room rule from its next question.

## Privacy

For an anonymous round, the server sends no per-voter data to anyone, the owner included:

- `RoundView.anonymous: true`; `result[].voter_ids` is empty (counts only); `ranking` is points, average place and first places per person (as always: never who ranked whom). `my_vote` / `my_ranking` still come back to the voter alone.
- `voter_ids` on the round (who has voted, for "waiting for…", nudges and skips) stays: it says who took part, not what they chose. Live `vote_cast` events carry the voter only, as before; pushes and inbox entries never name voters.
- History rows carry `anonymous`; their winners are aggregates. The CSV export writes `Ana (3)` instead of `Ana (3) : Bob, Carl` for those rounds.
- Stats (`/api/stats`, stat cards): an anonymous round counts as votes cast (per voter) and received (per person), wins, ties, unanimity and timing, but never as who-for-whom: no pairs, graph edges, duo, loyal, twins, self votes (narcissist), contrarian / sheep or kingmaker, no top voters / targets in profiles. Ranking averages are aggregates and stay.
- Share cards, challenges and podiums only ever show aggregates (top 3 by votes or points).

Every result read goes through `game::tallies` (voter ids dropped for an anonymous round) or `game::borda`. Features that must hide themselves on an anonymous round (a bet on the result, for instance) use `anonymous::is_anonymous(db, round_id)` or `rounds.anonymous` in SQL.

## App

- A small `Anonyme` pill with the lucide `Glasses` icon next to the question's tag, on the ballot and the result; the result's bars show `3 votes` instead of voters' faces. A `Glasses` mark on history rows, queue rows (under `per_question`), the import preview and pack questions.
- Composer: a `Glasses` switch beside the kind, only when the room rule is `per_question` and the question isn't a choose one.
- Room settings: `Votes anonymes : Jamais / Au choix / Toujours`, hidden for parties.

## Older apps and servers

- Older apps get empty `voter_ids` for anonymous rounds: their result bars simply show no faces. They never send the flag, so their questions aren't anonymous unless the room rule is `always`; editing a pack without the flag keeps the pack's.
- The new app shows nothing of this without `anonymous_votes` in `/api/features`.

## Storage

Migration 0045: `groups.anonymous_votes` (`never` | `per_question` | `always`, default `per_question`), `questions.anonymous`, `pack_questions.anonymous`, `rounds.anonymous` (all `0` by default).
