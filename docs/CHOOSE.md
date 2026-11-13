# "Qui choisirait quoi ?" questions

A third kind of question next to *pick* (one person) and *rank* (everyone in order): **choose**. A prompt and 2 to 4 items ("Qui prendrait quoi au resto ? Pizza / Sushi / Burger / Tacos"). Each voter wires every member of the room to an item: for themselves their **real** choice, for everyone else their **guess**. The result shows, per person, what the others thought they would take against what they really took, and who knows the group best.

## Authoring

- **Composer** (Questions tab): a third type next to *Classement*; 2 to 4 item fields.
- **Files and the built-in deck**: `[choix] Qui prendrait quoi au resto ? | Pizza | Sushi | Burger` (`[choose]` also works). The text before the first `|` is the question, each other part an item. Vibe tags combine as usual (`[choix] [soiree] …`).
- Limits: question 8 to 200 characters (as always); 2 to 4 items, each 1 to 40 characters, no two alike (case-insensitive). A file line that breaks them is skipped; the composer gets `400 bad_items`.
- Not in parties (they draw *pick* questions only), packs or deck suggestions for now.

## Storage

- `questions.kind` gains `'choose'` (migration 0027 rebuilds the table for the CHECK), `questions.items` holds the items as a JSON array of strings.
- `choices (round_id, voter_id, member_id, item)`: one row per voter and person wired, `item` = index in `items`. The `votes` row is still written (with `target_id` = the voter) so everything about who voted, waiting, nudges and closing works unchanged.

## API

- `POST /api/questions {text, kind: "choose", items: ["Pizza", "Sushi"]}`. `GET /api/questions` → `custom[].items` (`null` except for choose).
- `POST /api/round/current/vote {choices: [{member_id, item}]}`: every active member exactly once (`400 bad_choices` otherwise; `400 choices_needed` with no body).
- `RoundView` gains:
  - `items: string[] | null`
  - `my_choices: {member_id, item}[] | null` (my wires, once I voted)
  - `choose: ChooseResult | null` (closed rounds only):
    ```
    { people: [{ member_id, real: number | null, top: number[], guesses: [{ voter_id, item }] }],
      scores: [{ member_id, hits, total }] }
    ```
    `people` in member order: `real` = their own wire (null if they didn't answer with choices), `guesses` = the others' wires to them, `top` = the most guessed item(s) (empty when nobody guessed). `scores` = for each voter, how many of their guesses matched someone's real choice (`hits`) out of the guesses that could match (`total`); best first (hits, then ratio, then member id).
- History: a choose question's "winners" are the best guessers (`top_count` = their hits; none when nobody guessed right).
- Stats leave choose questions out for now (a "devin" award comes later).

## Older apps

New apps send `x-kinds: choose`. Without it, a choose question's text comes back as "Mets l'app à jour pour répondre : …" (English: "Update the app to answer: …"), and the single pick an older app sends is accepted as a vote without wires: nobody waits for them, it adds nothing to the result.
