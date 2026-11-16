# Quidenous · question pack template (for an AI)

This file tells an AI (ChatGPT, Claude…) how to write a **question pack** for Quidenous, an app where a group of friends answers one question a day: "Which of us…?". Read all of it, then write a `.txt` file that follows this format exactly. Reply with the file only, in a code block, with nothing around it.

## File format

A plain text file (UTF-8), **one question per line**. Blank lines are ignored.

### Header (top of the file)

`# key: value` lines, before the first question:

| Key | Value | Required |
|---|---|---|
| `# title:` | the pack's name, 40 characters max (short and catchy, ideally under 32) | yes |
| `# description:` | one sentence, 140 characters max (ideally under 90) | recommended |
| `# category:` | 1 to 3 categories from the list below, comma-separated | recommended |
| `# lang:` | `en` or `fr` (the language of the questions) | yes |
| `# 16+:` | `yes` or `no` | recommended |

Any other line starting with `#` is a comment (ignored).

### Categories

- `potes` (Friends): life in the friend group (habits, memories, flatshares, holidays).
- `pimente` (Spicy): flirting, love, couples, cheeky questions. **Always 16+.** Nothing explicit.
- `boulot` (Work): work, the office, studies, colleagues.
- `famille` (Family): family, family dinners and trips, childhood.
- `soiree` (Party): parties, nights out, the morning after.
- `culture` (Culture): films, music, travel, tastes, general knowledge.
- `absurde` (Absurd): wild hypotheticals, unlikely situations, absurd scenarios.

16+ rule: `# 16+: yes` when the pack has cheeky questions. A `pimente` pack is always 16+ (even if the line says `no`). Cheeky questions belong in `pimente` packs only.

### The three kinds of questions

1. **Which of us** (default): a plain line. Everyone votes for one person in the group.
   `Who would forget their passport on the day we leave?`
2. **Ranking**: the line starts with `[rank]` (or `[classement]`). Everyone ranks the group's members.
   `[rank] Rank us from always on time to always late.`
3. **Who would choose what**: the line starts with `[choose]` (or `[choix]`), then the question, then 2 to 4 answers separated by `|`. Everyone picks an answer and guesses the others'.
   `[choose] Your ideal dinner? | Pizza | Sushi | Fondue`

## Limits (otherwise the line or the pack is refused)

- Question: 8 to 200 characters (aim for under 100).
- `[choose]`: 2 to 4 answers, each 1 to 40 characters, all different.
- Pack: 5 to 300 questions (a good pack has 12 to 35). No duplicates.
- Title 40 characters max, description 140 max, 3 categories max.

## Tone

- Short, funny, kind. A question reads in one second.
- Talk to the group: "Who…", "Which of us…", "Rank us…".
- Never real names, and no real people or celebrities as targets.
- Nothing hurtful: no jokes about anyone's looks, weight, health, origin, religion, orientation or money.
- Keep the spicy stuff in `pimente` packs, with nothing sexually explicit.
- Mix: about two thirds "which of us", a quarter `[rank]`, a few `[choose]`.

## Example 1

```text
# title: Flatshare Chronicles
# description: Soaking pans, a shared fridge, passive-aggressive notes.
# category: potes
# lang: en
# 16+: no
Who leaves a pan "soaking" for four days?
Who sticks passive-aggressive notes on the fridge?
Who eats your cheese and swears blind it wasn't them?
Who never takes the bins out but notices when they overflow?
[rank] Rank us from loudest to quietest through a thin wall.
[choose] Your ideal spot in the flat? | The big bedroom | Next to the kitchen | The sofa
```

## Example 2

```text
# title: Love hot takes
# description: First dates, texting and grand gestures.
# category: pimente, soiree
# lang: en
# 16+: yes
Who would fall in love in a single evening?
Who rereads a text ten times before sending it to their crush?
Who has used a frankly questionable chat-up line?
[rank] Rank us from most romantic to most blunt.
[rank] Rank us from replies in 2 seconds to leaves you on read.
[choose] The perfect first date? | A drink | The cinema | A walk | Dinner
```

## Ready-to-paste prompt

> Read this template and write a Quidenous pack of 20 questions about [TOPIC] for a group of friends, in English. Follow the format exactly: full header (title, description, category, lang, 16+), about 13 "which of us" questions, 5 `[rank]` and 2 `[choose]` with 2 to 4 short answers. Reply only with the `.txt` file's content in a code block.

The file imports in the app: Packs → Publish a pack → .txt file (the header pre-fills the title, description, categories, language and 16+).
