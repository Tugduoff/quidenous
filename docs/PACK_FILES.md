# Pack files

A pack file is a UTF-8 `.txt` file: an optional header, then one question per line. The pack creator imports it
(Packs → Publier un pack → Fichier .txt), `POST /api/packs { text }` publishes it, `server seed-packs` publishes a
whole folder of them, and the room import (`POST /api/questions/import`, `server --import`) reads the same file and
skips every `#` line. One parser on each side: `server/src/packfile.rs` and `app/src/lib/pack-file.ts` (same rules).

An AI agent can learn the format from the template served at **`/pack-template.md`** (French) and
**`/en/pack-template.md`** (English) (`server/assets/pack-template.{fr,en}.md`, "Modèle pour IA" in the app). A test
parses its two example files.

## Example

```text
# titre: Chroniques de coloc
# description: Vaisselle qui trempe, frigo partagé, post-it passifs-agressifs.
# catégorie: potes, soiree
# langue: fr
# 16+: non
Qui laisse une poêle « tremper » pendant quatre jours ?
[classement] Classe-nous du plus bruyant au plus silencieux à travers une cloison.
[choix] Ta place idéale dans la coloc ? | La grande chambre | Près de la cuisine | Le canapé
```

## Header

The `#` lines at the top, before the first question (blank lines allowed). Each is `# key: value`; keys are read
case- and accent-insensitively. After the first question, `#` lines are plain comments.

| Key | Also | Value |
|---|---|---|
| `titre` | `title`, `nom`, `name` | the pack's name (40 characters max) |
| `description` | `desc` | 140 characters max |
| `catégorie` | `categorie`, `categories`, `category`, `tags`, `tag` | 1 to 3 categories, comma- or `;`-separated: ids (`potes`, `pimente`, `boulot`, `famille`, `soiree`, `culture`, `absurde`) or their French / English names (`Entre potes`, `Pimenté 16+`, `Soirée`, `Friends`, `Spicy`, `Work`, `Family`, `Party`, `Absurd`…). Unknown ones are ignored, the first 3 kept. |
| `langue` | `lang`, `language` | `fr` / `en` (`français`, `english`… also read) |
| `16+` | | `oui`/`non`/`yes`/`no` (`true`/`false`/`1`/`0`). Always yes with the `pimente` category. |

Unknown keys (`# taille: small`) and `#` lines without a known key are comments. An empty or unreadable value is
ignored; when a key repeats, the last one wins.

- **Pack creator**: importing a file fills the name, description, categories, language and 16+ from its header (all
  still editable) and adds its questions.
- **`POST /api/packs { text }`**: the header fills what the body leaves out (`name`, `description`, `tags`,
  `language`, `spicy`); body fields win.
- **Room import**: the header is ignored (comments).

## Questions

- A plain line is a *pick* question ("qui de nous").
- `[classement] …` (or `[rank]`) is a ranking question.
- `[choix] Question | A | B | C` (or `[choose]`) is a "qui choisirait quoi" question with 2 to 4 answers
  (docs/CHOOSE.md).
- `[quiadit]` (or `[said]`) lines (« Qui a dit ça ? », docs/SAID.md) are read by the room import and the built-in deck, but packs leave them out for now (a pack file's said line counts as dropped).
- `[anonyme]` (or `[anonymous]`) asks for anonymous votes on a pick or ranking question (docs/ANONYMOUS.md); it
  combines with the others in any order (`[classement] [anonyme] …`) and is dropped on a choose question.
  `pack_questions.anonymous` (migration 0045) keeps it, `GET /api/packs/{id}` lists `questions[].anonymous`,
  `POST`/`PUT` accept `anonymous` (absent: an older app keeps what the pack had), installing carries it onto the
  room's questions.

Limits: question 8 to 200 characters; answers 1 to 40 characters, 2 to 4, no two alike; 5 to 300 questions per pack,
no duplicates (case-insensitive). Lines that break them are left out.

## Choose questions in packs (`pack_choose`)

`pack_questions.items` (migration 0036) holds a choose question's answers (JSON array). `GET /api/packs/{id}` lists
`questions: [{ text, kind, items }]`, `POST`/`PUT /api/packs` accept `items` with `kind: "choose"`, and installing a pack
creates real choose questions in the room. Older apps: a choose question sent back without `items` (an app from before
`pack_choose` editing a pack) keeps the answers the pack has; one the pack doesn't know is left out. In a room, older
apps show choose questions as "update the app" (docs/CHOOSE.md). A new app on an older server leaves choose questions
out of what it publishes (no `pack_choose` in `/api/features`), and hides the AI template link without `pack_template`.

## Admins

Admins (`ADMIN_USER_IDS`) have no limit of 10 packs, never publish into review, and may send `featured: true` when
publishing (Sélection Coflop); others' `featured` is ignored.

## Seeding (`server seed-packs`)

```bash
server seed-packs <dir> --author-id <user id> [--featured] [--dry-run]
```

Publishes every `*.txt` under `<dir>` (sorted, subfolders included) as the author's public packs, with no pack limit.
Each file needs `# titre:` and `# langue:`. A pack whose title (case-insensitive) and language the author already has
(not deleted) is skipped, so running it again is safe. `--featured` puts the new packs in Sélection Coflop;
`--dry-run` reports without writing. Exit code 2 when a file is invalid (the others are still published).

The first community packs (84 files: 42 packs in French and English, 7 categories) live in `server/packs/` and ship in
the Docker image as `/packs`:

```bash
docker compose exec quidenous server seed-packs /packs --author-id 2 --featured --dry-run
docker compose exec quidenous server seed-packs /packs --author-id 2 --featured
```

A test loads them all with the real parser (every line valid, FR/EN twins with the same kinds).
