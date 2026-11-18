# Le Tripot

Bets on who wins a question, with chips (`jetons`) that are worth nothing. Feature flag `tripot` in `GET /api/features`.

## No real money, ever

- Chips can't be bought, earned through ads, traded, transferred to someone else or turned into anything. There is no in-app purchase, no reward, no prize. The only way to get chips is to vote (and to win bets).
- They are per room and reset every month: nothing piles up.
- The bet sheet says so ("Jetons sans valeur, juste pour le jeu." / "Chips have no value, just for fun."), so does the stats board.
- Google Play: this is *simulated gambling* in the IARC questionnaire (Play Console > Content rating). Answering yes there likely raises the rating (PEGI 12 / Teen range). The data model has no age or minors flag (no birth date anywhere), so Tripot can't be hidden for minors; the owner can turn it off per room.

## Chips

- Per member, per room, per calendar month (UTC, `YYYY-MM`). Everyone starts each month with **100**.
- **+20** on the first vote of each UTC day in that room (any question kind, old apps included), granted inside the vote's transaction. The bonus never takes a balance above **500** (it tops up to 500; at 500 a `0` row still marks the day). The day is the plain UTC date, not the member's time zone like the streaks (docs/STREAKS.md): a simpler rule that can't be gamed by changing one's zone, and that matches the UTC month.
- **On the 1st** everyone is back to 100: a balance is `100 + SUM(amount)` of that month's ledger rows, so nothing has to run at midnight.
- `tripot_ledger` is append-only: `daily` (+20, or less at the cap), `stake` (−amount), `payout`, `refund`, each with its `month` and, for bets, its `round_id`. Every number shown can be recomputed from it.
- A bet's payout or refund goes back to the month its stake came from (`tripot_bets.month`): a bet placed on the 31st and settled on the 1st counts for last month's podium, and doesn't add to the new month's 100.

## Bets

- On an **open pick or ranking question of a room**: who gets the most votes (ranking: who ends up first, Borda points as the result shows them). Not on choose or said questions (docs/SAID.md), not in parties, not on anonymous questions (`rounds.anonymous`, docs/ANONYMOUS.md): there the API sends nothing about Tripot at all.
- One bet per member per question; no change, no cancel. Amount 1 to my whole balance (the app offers 10 / 25 / 50 / Tapis). Betting on yourself is allowed, before or after voting.
- Bets close when the question closes.
- **While open** everyone sees only the pot and the number of bettors, plus their own bet. Never who bet on whom, nor anyone else's stake.

## Settlement (pari-mutuel)

In the close path (`game::close_if_done` → `tripot::settle`, same transaction as the close; bets already settled are skipped, so it is idempotent):

- Winners: the members with the most votes (pick) or the most points (ranking); ties: every tied member wins.
- All stakes form the pot. Bets on a winner share it in proportion to their stakes, rounded down; the chips left over go one each to the winning bets by largest stake, then earliest bet. The house takes nothing.
- Nobody bet on a winner, or no winner at all (a ranking nobody ranked) → every stake is refunded.
- A question removed while open (`game::drop_round`, "Question retirée") → refunded. A question retired after it closed keeps its settlement.
- Closed, the result shows the pot and the winning bettors with their stake and winnings (`winners`), or `refunded`.

## Room setting

`groups.tripot` (default on). Owner: Paramètres du salon > Votes > "Tripot (paris en jetons)", `PATCH /api/rooms/{id} {tripot: bool}` (`400 bad_setting` on a party). `GET /api/group` → `settings.tripot` (rooms only). Off: no bets, no daily bonus, nothing shown, `GET /tripot` answers `403 tripot_off`; the open question's bets are refunded and dropped (back on, everyone may bet on it again). A new anonymous rule that makes the open question anonymous does the same to its bets.

## API

- `RoundView.tripot` (absent where Tripot doesn't apply): `{ pot, bettors, my_bet: {target_id, amount, payout} | null, balance, can_bet, winners: [{member_id, stake, won}] | null (closed only), refunded }`. `balance` is mine this month.
- `POST /api/rounds/{id}/bet {target, amount}` → the round's `tripot`. Errors: `404` (not a question of my room), `403 tripot_off`, `409 round_closed`, `400 no_bets` (choose / anonymous), `400 unknown_member`, `400 bad_amount`, `409 not_enough_chips`, `409 already_bet`. Live event `tripot_updated {round_id}` (no member in it).
- `GET /api/tripot` → `{ month, last_month, start: 100, daily: 20, cap: 500, balances: [{member_id, balance}] (current members, most first), podium: [{member_id, balance}] (last month's top 3 among current members who played then) }`.

## App

- Ballot and waiting screen: a compact row with the lucide `Coins` icon, "Parier" → a small sheet (who, for a ranking "qui finira premier", stake chips, the "jetons sans valeur" line, confirm). Afterwards the row says "Mise : 25 sur Bob" with the pot and bettors.
- Result: a card with the pot and who won how much, a short shower of gold coins (about a second; nothing under reduced motion) and my own outcome.
- Stats: "Tripot" board (this month's chips) with last month's podium on top.
- Shown only with `tripot` in `/api/features` and `settings.tripot` (the round only carries `tripot` where it applies). Older apps ignore the new fields; an unknown live event makes them refresh once.

## Storage

Migration 0047: `groups.tripot`, `tripot_bets` (one per member per round, `month`, `payout`, `refunded`), `tripot_ledger` (append-only, one `daily` row per member per UTC day).
