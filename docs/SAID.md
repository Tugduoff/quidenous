# « Qui a dit ça ? » questions

Status: built, 2026-11-18. Server: `server/src/said.rs`, migration `0051_said.sql`, tests `server/tests/said.rs`. App: `components/said.tsx` (room), `components/party-said.tsx` (party phone and big screen), `lib/said.ts`, locales `said.ts`. JS-only (ships OTA).

A fourth kind of question next to *pick*, *rank* and *choose*: **said**. A prompt everyone answers in free text (« Le pire cadeau qu'on t'ait fait ? »). The answers stay hidden and anonymous while people write; once the room is done writing, everyone gets the answers in a random order and guesses who wrote each one. The reveal shows the authors one by one, with points for finding people and for fooling them. Feature flag `said` in `GET /api/features`.

## Flow

One round, three phases (`RoundView.said.phase`):

1. **writing** (round open, `rounds.said_guess_at` null). Each member writes one answer (`POST /api/round/current/answer {text}`), and can rewrite it until the writing ends. Everyone sees *who* has written (`said.writer_ids`), never what.
2. **guessing** (round open, `said_guess_at` set). Starts when the people the question waits for have all written (same set as for closing: active members minus the skipped and the away, docs/ROOMS.md; while writing, a skip or a nudge targets someone who hasn't written), or once the question has been open for `QUORUM_HOURS` (24 h) and half of them wrote, **and** at least 2 real answers exist. With fewer (say one answer and older apps' passes), the question closes with no result instead (`phase: "done"`, `guess_at`, `answers` and `result` null), as at a party's writing deadline: a room where only older apps play never gets stuck. Skips on people who wrote are dropped when the guessing starts (they were about the writing). From then on answers are frozen (`409 not_writing`). Each member, author or not, matches every answer except their own to its author among the writers except themselves (a one-to-one matching) and sends it as their vote. Members who wrote nothing can still guess.
3. **done** (round closed). Closes with the usual rule (everyone waited for voted, or 24 h after the guessing started and half of them did; checked by the same scheduler as the late quorum). The result names every author.

Points (computed at read time, never stored):

- **found**: +1 to a guesser for each answer they matched to its real author.
- **fooled**: +1 to an author for each guesser who did not find them.

The winners (history, share card, inbox "result") are the top scorers (`points > 0`; ties share).

**Rooms of 3 or more.** With two people there is nothing to guess: a said question is only drawn (room queue or deck) when the room has at least 3 active members; at a party, at least 3 players. A room's own said question waits in the queue while the room is smaller (the next one is drawn instead).

## Storage (migration 0051, number may need bumping when merged)

- `questions.kind` gains `'said'` (the table is rebuilt for the CHECK, as in 0027).
- `rounds.said_guess_at TEXT`: when the guessing started (null while writing; null for other kinds).
- `said_answers (id INTEGER PRIMARY KEY, round_id, member_id, text, created_at, hidden_at)`, `UNIQUE (round_id, member_id)`. `id` is random (53 bits from the server's RNG), never sequential, so neither ids nor the order by id say who wrote first; the answers are listed by id (a random order fixed for the round). `text` null = a "pass" from an older app (see below): counts as written, shows nothing, is not guessable (not listed in `answers`). `hidden_at`: hidden by reports; an admin's delete (and the author's account deletion) also blanks `text` to `''`.
- `said_guesses (round_id, voter_id, answer_id, member_id)`, primary key `(round_id, voter_id, answer_id)`.
- `reports.kind` gains `'answer'` (table rebuilt for the CHECK, as in 0033).
- The `votes` row is still written when someone sends their guesses (`target_id` = the voter, as for choose), so who voted, waiting, skips, nudges, streaks and closing work unchanged.

## API

- `POST /api/questions {text, kind: "said"}` (composer). `400 room_too_small` never: the question can wait. Files and the built-in deck: `[quiadit] Le pire cadeau qu'on t'ait fait ?` (`[said]` also works; combines with vibe tags). Never anonymous (the tag is dropped), not in packs for now.
- `POST /api/round/current/answer {text}`: 1 to 140 characters after `text::line` cleaning. Errors: `400 bad_answer` (empty / too long), `409 no_open_round`, `409 not_said` (open question is another kind), `409 not_writing` (guessing started), `409 time_up` (party, past the writing deadline), `409 joined_late` (party), `410 party_over`. Answer: the round view. Rewriting replaces the text (same answer id). Writing after an older app's pass replaces the pass (and its vote: the guessing waits for this member's guesses). Fires a live `vote_cast` (below) and may start the guessing.
- `POST /api/round/current/vote {guesses: [{answer_id, member_id}]}` (guessing only): exactly one guess per answer that isn't mine and isn't a pass, every `member_id` a writer other than me (`writer_ids`, passes included), no member twice. `400 guesses_needed` (no `guesses` on a said question), `400 bad_guesses`, `409 not_guessing` (still writing), `409 already_voted`.
- `RoundView` gains `said: SaidView | null` (said questions only):

```
{
  phase: "writing" | "guessing" | "done",
  guess_at: string | null,             // when the guessing started
  writer_ids: number[],                // who has written (passes included), member order
  my_answer: { id: number, text: string } | null,
  answers: [{ id: number, text: string | null, hidden: boolean }] | null,
                                       // guessing and done: every real answer, random fixed order,
                                       // mine included (the app knows it by my_answer.id);
                                       // hidden = 3 reports (text null)
  my_guesses: [{ answer_id, member_id }] | null,   // once I sent them
  result: {                            // done only
    answers: [{ id, author_id, found: number, fooled: number,
                guesses: [{ voter_id, member_id }] }],   // reveal order: most found first, best hidden last
    scores: [{ member_id, points, found, fooled }]       // best first (points, then found, then member id)
  } | null
}
```

  `voter_ids` keeps meaning "sent their guesses" (plus older apps' passes and votes). `my_vote` is my own id once I guessed. `my_answer` keeps my own text even when the answer is hidden (I wrote it); a pass gives `my_answer: null` (I am still in `writer_ids`).
- History: a said question's winners are the top scorers, `top_count` their points (none for a question that closed before any guessing). The CSV export, the share card and a shared question's podium list the top scorers too.
- Report an answer: `POST /api/reports {kind: "answer", id: <answer id>, ...}` (same rules as a message: from inside the room, not my own (`400 bad_target`), 3 trusted reporters hide it until an admin decides; the admin's `dismiss` brings it back, `delete` blanks it for good). Only once shown (guessing or done; `404` before, and for a blanked one). A hidden answer stays in the game (it was written, its guesses and points count) but shows as hidden. Hiding, bringing back and deleting send `round_updated`.
- Account deletion blanks the person's answers like an admin's delete; leaving (or being removed) while the question is open takes their answer away with the guesses on it, and their own guesses.

## Live

- Writing an answer sends `vote_cast { round_id, voter_id }` (the writer): apps refetch the round, "who has written" updates. Older apps just refetch.
- The guessing starting sends `round_updated { round_id }` (new event; older apps ignore it, they refetch on the next one). So do an answer hidden by reports, brought back or deleted by an admin.
- A push / inbox `question` entry tells everyone in a room (not a party) that the answers are in: « Les réponses sont là : qui a dit quoi ? » / "The answers are in: who said what?".

## Party

- Parties draw said questions too (with 3 players or more), from the room's own queue and the deck, like pick questions.
- Writing deadline: `max(60, 2 × timer)` seconds; guessing deadline: `max(45, 2 × timer)`. `rounds.deadline_at` holds the current phase's deadline. At the writing deadline with 2 answers or more the guessing starts (new deadline); with fewer the round closes with no result.
- `GET /parties/{id}` and the screen view: `round.kind`, and `round.said` (the same shape; on the screen view `my_*` are null and `result` is there at the reveal). `POST /parties/{id}/answer {text}` (answers the party view), `POST /parties/{id}/vote {guesses}` (`target_id` is optional now; same errors as the room endpoints). `round.voted` is who sent their guesses (and older apps' passes); who wrote is `round.said.writer_ids`.
- The deadline task: when the guessing starts (deadline or everyone wrote), a task sleeps until the new deadline; the 5 s sweeper catches anything missed.

## App

- Room (`SaidRound`): writing (answer field, then my answer with *Modifier*; who has written), guessing (Zeste, a match board: tap an answer, then its author; long press reports), then the reveal: authors one by one (who was named, the author flips, points), then the scoreboard and every answer with its author.
- Party phone: the same pieces with the countdown (`phaseSeconds()` gives each phase's length); someone who joined mid-question plays the next one.
- Big screen: writing like a pick question; guessing lists the answers big, numbered and anonymous (two columns, no QR panel: the code moves to the header); the reveal plays by itself from `closed_at` (about 6-7 s per answer, so several screens stay in step), then the scoreboard. A round that closed with too few answers shows a short line.
- The composer offers « Qui a dit ça ? » only when the server lists `said` in `/api/features`.

## Older apps

New apps send `x-kinds: choose,said`. Without `said`, the text comes back as "Mets l'app à jour pour répondre : …", and the single pick an older app sends counts as a pass while writing and as a vote without guesses while guessing: nobody waits for them.

## Streaks and reminders

- A written answer counts like a vote for streaks (that day, that question). The evening reminder skips someone who wrote while the question is still in its writing phase; once the guessing started, it waits for their guesses.

## Not in

- Stats leave said questions out (like choose); a "détective" award can come later.
- Tripot bets and anonymous votes don't apply to said questions.
- Packs don't carry said questions yet.
