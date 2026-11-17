# Google Play listing: qui de nous ?

Everything to copy into the Play Console. Package `com.tugduoff.quidenous`. Nothing in this folder ships in the app.

## Files

| File | What | Size |
| --- | --- | --- |
| `short-description.txt` | Short description (FR), 71 / 80 chars | 73 B |
| `full-description.txt` | Full description (FR), 3953 / 4000 chars (`wc -m`, v0.39.0) | 4.0 KB |
| `icon-512.png` | App icon, 512x512, 32-bit PNG, full-bleed square (Play applies the mask) | 100 KB |
| `feature-graphic.png` | Feature graphic, 1024x500, 24-bit PNG, no alpha | 141 KB |
| `screenshots/01-vote.png` | Phone, 1080x1920: voting (world question card, Tripot "Parier" row, pick grid) | 136 KB |
| `screenshots/02-result-tripot.png` | Phone, 1080x1920: result with who voted for whom + Tripot winnings | 144 KB |
| `screenshots/03-chat.png` | Phone, 1080x1920: chat under a result, with a photo | 152 KB |
| `screenshots/04-ranking.png` | Phone, 1080x1920: "Classe-nous" ranking result | 136 KB |
| `screenshots/05-world-question.png` | Phone, 1080x1920: Question du monde, room tally + anonymous facts across rooms | 140 KB |
| `screenshots/06-party.png` | Phone, 1080x1920: party mode result (guests without accounts) | 116 KB |
| `screenshots/07-packs.png` | Phone, 1080x1920: community packs | 148 KB |
| `screenshots/08-streaks-tripot.png` | Phone, 1080x1920: stats, streak records + monthly Tripot board | 128 KB |

Upload order = file order. History and the plain stats/awards screens were dropped for the new features (8 is the Play maximum).

Screenshots are the real v0.39.0 web build (same code as the app) in the dark theme, 405x720 CSS px at 8/3 = 1080x1920, against a throwaway local server with a demo room (Léa, Hugo, Inès, Nathan, Camille, Jules; party guests Zoé, Malik, Sacha), two weeks of play back-dated in SQLite for streaks and stats, the 42 Sélection Coflop packs seeded, and filler rooms so the world question has enough rooms. The chat photo is a drawn voice-note picture. No real user data.

### Icon and feature graphic (checked at v0.39.0)

- `icon-512.png` is the launcher icon scaled down: lime `?` (#C8FF3D, Bricolage Grotesque, soft glow) on #0E0E12, same art as `app/assets/images/icon.png` (1024 px), the adaptive icon (`app.json`: background #0E0E12) and the splash. The stock Expo icon is long gone (replaced in #10, full-size fix in 5b4a5be).
- `feature-graphic.png` matches the app: #0E0E12 background, the `qui de nous?` wordmark in Bricolage Grotesque ExtraBold with the lime `?` (same as the in-app `Logo`), the big glowing `?` of the icon, and six avatar initials (L H I N C J) from the screenshot demo room. Its only claim, "Tes potes votent. Le verdict tombe.", is still true. Nothing to redo. The new screenshots keep the same demo names, so the initials still match. If you want the graphic to sell the new features, a variant could swap the initials for a small "séries · Tripot · soirées" line. Keep it short: Play advises against much text on feature graphics.

## Store listing

| Field | Value |
| --- | --- |
| App name | `qui de nous ?` |
| Default language | French (France), fr-FR |
| App or game | **Game**, category **Casual** (recommended) |
| Tags | Party, Multiplayer, Casual (pick from the Console list) |
| Contact e-mail | contact@coflop.fr |
| Website | https://qdn.coflop.fr (optional) |
| Privacy policy URL | https://qdn.coflop.fr/privacy |
| Account deletion URL (Data safety) | https://qdn.coflop.fr/delete-account |

Category: Casual game rather than Social. The description says "jeu entre amis", people look for "jeu entre potes / qui est le plus susceptible de" in Games, and Casual is where most "most likely to" apps sit. Social would also be defensible (private rooms, chat, accounts) but competes with messaging apps. Either way the IARC answers below are the same.

Wording, checked against v0.39.0:

- Pace: the description says "une question à la fois ... une par jour ou dix d'affilée". The server opens the next question as soon as everyone has voted, and the owner can pace it (only the owner launches the next one, or a delay of up to 24 h: docs/ROOMS.md, Pacing). The terms now say "une question à la fois" (not "une question par jour" any more): consistent, nothing to change.
- Votes: secret while the question is open. Then the result shows who voted for whom, **except on anonymous questions** (docs/ANONYMOUS.md): the owner sets "Votes anonymes : Jamais / Au choix / Toujours", and an anonymous result only shows totals, to everyone, the owner included. The privacy page's "votes selon les règles du salon" now matches that exactly. The description has its own "VOTES ANONYMES" section.
- 16+: the terms ("Âge": "réservée aux personnes de 16 ans et plus", confirmed when accepting) and the privacy page ("Âge") both say 16+, in French and English, and the "16+" packs and Pimenté vibe ask for an extra confirmation. The description ends with "Réservé aux 16 ans et plus."
- Tripot: the description says the chips are fictional, can't be bought or cashed, involve no real money, and that the owner can turn it off. All true (docs/TRIPOT.md); the app says so on the bet sheet ("Jetons sans valeur, juste pour le jeu.") and on the stats board.
- "Question du monde" is the in-app name (`globalQuestion.label`), not "question mondiale". Its cross-room numbers need 5 rooms that answered that day; with few users the screen says "Pas encore assez de salons". The description doesn't promise numbers.
- "Plus de 250 questions": the built-in decks have 266 (FR) and 324 (EN) questions; four room vibes (Entre potes, Pimenté 16+, Boulot, Famille, `server/deck/vibes.json`).
- Google and Discord sign-in only show when their OAuth clients are configured on production (`oauth_google` / `oauth_discord` in `/api/features`). The description promises both: check that production lists both, or drop the one that's off.
- GIFs only exist with `KLIPY_API_KEY` set on production (`chat_gifs`): same check.
- **The privacy policy is behind the app**: it doesn't mention chat photos, Klipy, the time-zone offset, the Tripot or the global question. See docs/PLAY_DATA_SAFETY.md, "Policy-side items still open". Fix it before submitting.

## Content rating (IARC questionnaire)

Category: **Game** (or "Social networking / UGC" if you list it as Social; the answers are the same). Re-checked at v0.39.0: answer for everything a signed-in user can reach, including the opt-in Pimenté 16+ deck and the Tripot.

| Section | Answer | Why |
| --- | --- | --- |
| Violence (realistic, fantasy, blood, gore) | **No** | Text only. Some built-in questions mention prison, zombies, a serial killer, torture (FR only), as jokes; no depiction. GIFs and photos are user content (rows below). |
| Fear / horror | **No** | |
| Sexuality: nudity | **No** | None built in. User photos and GIFs: covered by the UGC answers; the terms ban explicit sexual content and the Klipy filter is on. |
| Sexuality: sexual innuendo / references | **Yes, mild, text only** | Default deck: "Qui pourrait coucher avec quelqu'un pour une promotion ?" (FR, untagged, so it is in the default Entre potes vibe; the EN line is the softer "flirt their way to a promotion"), cheating. Pimenté 16+ (opt-in vibe, 34 FR / 39 EN questions): "proposition indécente", "strip-poker", "coup d'un soir", "message coquin", "bisou pour gagner un pari". |
| Language (profanity, crude) | **Yes, mild** | "connerie" in a built-in question; user chat is unfiltered (covered by the interaction questions). |
| Crude humour | **Yes** | The whole game is teasing friends. |
| Controlled substances (drugs, alcohol, tobacco) | **Yes: alcohol references, text only** (was No) | "Qui commande quoi au bar ? Bière / Vin / Cocktail / Soft" (FR and EN choose question, since #30), "Who is most likely to send the group chat a drunk voice note?" (EN). No drugs, no tobacco, no use shown. |
| Gambling: real money | **No** | Nothing can be bought or cashed out, no prizes (docs/TRIPOT.md). |
| Gambling: **simulated gambling** | **Yes** (was No) | Le Tripot: players stake chips on who wins a question, odds come from the pot (pari-mutuel) and winners take it. The chips are free and worthless, but every authority's definition is about the activity, not the money (see below). Also text mentions: casino, loto, crypto, "un bisou pour gagner un pari". |
| Discrimination / hate | **No** | (See the note below on one question.) |
| Users can interact or communicate | **Yes** | Chat under every result inside a room: text, photos, GIFs, reactions, mentions. Party guests join a party by code or QR without an account and show up by first name to the party's players. |
| User-generated content shared with other users | **Yes** | Room members see custom questions, chat text, **photos** (user-uploaded, stored on our server) and **GIFs** (picked from Klipy), profile name and photo. All signed-in users see public question packs, creator pages (name, photo, packs) and deck suggestions (with first name); public `/p/…` pack pages and `/s/…` share links are open to anyone with the link. The global question only shows other rooms as anonymous percentages (5 rooms minimum): not UGC from strangers. |
| Moderation tools | **Yes** | Report a message (photos and GIFs included), question, member, pack or suggestion, with a reason; block a user. Three reports from trusted accounts hide a message, send a pack to review or pull a suggestion. Every report is e-mailed to the team; admins review them in the app (dismiss, delete, suspend, erase the account). Owners delete messages, restore hidden ones, remove members, lock a party; members can vote someone out. GIF search goes through Klipy's content filter (`KLIPY_CONTENT_FILTER`, default `low` ≈ PG-13). The terms list what's banned. |
| Shares the user's location | **No** | Only a time-zone offset is stored (quiet hours, streaks); nothing is shown to others. |
| Digital purchases / loot boxes | **No** | No in-app purchases, no random paid items. |
| Unrestricted internet access / web browser | **No** | The browser only opens for Google/Discord sign-in and our legal pages. GIF search returns Klipy results through our server, filtered. |
| Is the app a web browser or search engine | **No** | |

### Simulated gambling: why Yes, and what it does to the rating

The Play Console's own help text for this question is only visible inside the questionnaire; what follows is from the rating authorities' published definitions (checked 2026-11-17).

- **ESRB**: "Simulated Gambling: Player can gamble without betting or wagering real cash or currency. Typically assigned to simulations of casino-based gambling." Teen covers it ("may contain ... simulated gambling"). Source: https://www.esrb.org/ratings-guide/
- **Australia (ACB)**: since 22 September 2024, "Computer games containing simulated gambling have a minimum classification of R 18+". The 2023 guidelines define it as activity that "resembles or functions like a real world age restricted betting or gambling service" (examples include sports and other betting services, not only casinos) with no real-money payout. Betting chips on an outcome with pool-based odds fits. Sources: https://www.globalratings.com/ratings-guide.aspx, https://legislation.gov.au/F2023L01424/asmade/2023-10-27/text/original/pdf
- **PEGI**: "If a game contains themes that prominently reference gambling, it gets a PEGI 12. Simulation of gambling, like games of chance that are normally carried out in casinos or gambling halls, will be classified with a PEGI 18." (https://pegi.info/what-do-the-labels-mean). IARC's ratings guide still lists simulated gambling under PEGI 12. The Tripot is not a casino game of chance (no cards, slots or roulette, the outcome is a friends' vote), so expect **PEGI 12**; if the questionnaire asks a follow-up about casino-style games, answer No.
- **USK**: case by case (Leitkriterien 2022, 4.16 "Glücksspiel"; descriptor "Glücksspielthematik"); non-realistic stakes lower it. Likely 12; not verifiable without running the questionnaire.
- **Brazil (ClassInd, 2025 guide)**: features that "show odds, rankings or results that reproduce the logic and dynamics of gambling" are rated 18 even without real money. Likely **18** (our reading, not confirmed).
- **IARC generic**: 12+ ("mild language and simulated gambling are also permitted").
- **Google Play**: Families policy bans "real or simulated gambling" in apps aimed at children: irrelevant at 16+, but it rules out ever adding under-13s. In the EEA, Australia, Brazil, Indonesia, Singapore, Switzerland and the UK, Play blocks users it determines to be minors from getting 18+ rated apps: Australian (and probably Brazilian) minors can't install the app. Answering No to hide this is not an option: misrepresenting content can get the app removed (https://support.google.com/googleplay/android-developer/answer/188189).

Other PEGI criteria added in 2026 that the IARC questionnaire may now ask about (we couldn't see the current questionnaire): "pressure to play" (daily rewards or streaks, PEGI 12 when missing a day loses status: our streaks end on a missed day, and the first vote of the day earns 20 chips); "unrestricted communication" (PEGI 18 only when there is no block, mute or moderation: we have all three). Answer them truthfully if they show up.

Expected result: **PEGI 12** (16 is possible from the sexual references), **USK 12 to 16**, **ESRB Teen** (Simulated Gambling, Suggestive Themes, Crude Humor, Mild Language, Alcohol Reference; interactive element "Users Interact"; not "Shares Info", which means sharing personal data with third parties), **IARC generic 12+ to 16+**, **Australia R 18+** (because of the Tripot), **Brazil likely 18**. The computed rating can be lower than the 16+ target audience: that's fine, the target audience setting below is what keeps it away from kids. If R 18+ in Australia and 18 in Brazil are a problem, leave those countries out of the release (the app is French and English, France first), or turn the Tripot into fixed-point predictions with no stakes or pot.

Built-in questions worth softening in `server/deck/` (they drive the rating and could draw a policy flag). None of the four flagged on 2026-11-12 was softened in French:

- "Qui se ferait arnaquer par un faux prince nigérian ?" (fr.txt:28) and "Who would fall for a Nigerian prince scam?" (en.txt:28): national stereotype, in the default vibe, in both languages. The one most worth changing.
- "Qui pourrait coucher avec quelqu'un pour une promotion ?" (fr.txt:91, default vibe): the English version already says "flirt their way to a promotion"; do the same in French, or tag it `[pimente]`.
- "Qui serait le premier à dire oui à une proposition indécente ?" (fr.txt:134, en.txt:133): already tagged `[pimente]` (16+ only), fine as is.
- "Qui serait le premier à céder sous la torture ?" (fr.txt:21, default vibe): no English counterpart; soften or drop.

## Target audience and content

| Question | Answer |
| --- | --- |
| Target age groups | **16-17 and 18+** only (unchanged) |
| Appeals to children? | No |
| Designed for children / Families program | No (and can't be: the Families policy bans simulated gambling, i.e. the Tripot) |
| Contains ads | **No** |
| In-app purchases | **No** (free, nothing to buy; Tripot chips can't be bought) |
| News app | No |
| COVID-19 contact tracing | No |
| Government app | No |
| Financial features | None |
| Health features | None |
| Advertising ID (App content) | **No** (no ads or analytics SDK). Check that the AAB manifest has no `com.google.android.gms.permission.AD_ID` (a Firebase dependency can add it); if it does, either remove it in `app.json` or declare its use honestly. |

Still 16+ at v0.39.0, checked:

- Terms and privacy pages (`server/assets/legal/{terms,privacy}{,.en}.html`, version 1 of 16 November 2026): "L'app est réservée aux personnes de 16 ans et plus", confirmed when accepting the terms.
- In-app age gate: a self-declared checkbox, "J'ai 16 ans ou plus et j'accepte les conditions et la politique de confidentialité" / "I'm 16 or older and I accept the terms and the privacy policy", required on e-mail sign-up, the first-name join / new room screen and the party guest join; Google / Discord sign-ups and older accounts get the same box once in a sheet (`TermsGate`, docs/AUTH.md, "Terms and age"). No birth date anywhere, so nothing can be hidden per age: the Tripot is on for every room unless its owner turns it off.
- Second confirmation for 16+ content: the packs store's "16+" chip and the Pimenté 16+ vibe ask first ("Contenu 16+ : Des questions osées, réservées aux 16 ans et plus." > "J'ai 16 ans ou plus"); 16+ packs never show on public `/p/…` pages.

## Data safety

The answers live in [docs/PLAY_DATA_SAFETY.md](../../docs/PLAY_DATA_SAFETY.md), re-checked at v0.39.0. New since the first draft: chat photos (Photos), GIF and pack search (In-app search history, ephemeral, Klipy as a service provider), time-zone offset and language (Other info), Tripot bets and global question votes (App interactions), crash reports from any unexpected error with the purpose moved to Analytics, and the privacy policy gaps to close before submitting.

## App access (for reviewers)

Production runs with `AUTH_REQUIRED=1`: the old anonymous ways in (`POST /api/join` by first name, `POST /api/start`) answer `410 sign_in_required` (`routes::legacy_gate`). Signing in is the only way into a room; the only account-less path left is joining a **party** as a guest (`POST /api/parties/join`, not gated, rate-limited per IP). So declare **"All or some functionality is restricted"** and give a login.

What a reviewer goes through at v0.39.0 (`app/src/app/join.tsx`, `auth/*`): Welcome screen > "Continuer avec Google / Discord" (when configured) or "Continuer avec un e-mail" > e-mail > Continuer > the password screen: an existing account just signs in ("Se connecter"); a new one asks first name + password (8 characters minimum) + the 16+ terms checkbox ("Créer mon compte"), then a 6-digit e-mail code that can be skipped ("Plus tard"). No 2FA unless the account turned it on, no onboarding tour. An account with no room lands on "Ton salon" (join with a code or create one). The app follows the phone language: French phones get French, any other language gets English (Profil > Langue to change it). Tabs at the bottom are icons only (Vote, Historique / History, Stats, Questions), in that order.

Requirements that shape the demo room: the global question needs a room of **3 people or more** (cross-room numbers need 5 rooms that answered that day, so expect "Pas encore assez de salons" / "Not enough rooms yet"); Tripot bets need an **open pick or ranking question that isn't anonymous**, and everyone starts each month with 100 chips (no voting days needed); streak flames show from **2 days** in a row (calendar days, so they take real days to build); a party needs **2 players** to start; publishing, liking or reporting a pack needs a **verified e-mail**.

Nobody created anything on production. To do yourself, after v0.39.0 is deployed, starting **3 or 4 days before you submit** (for the streaks):

1. **Accounts** (e-mail sign-up in the app or on https://qdn.coflop.fr, tick the 16+ box, enter the e-mailed code so each one is verified: the addresses must reach a mailbox of yours, e.g. aliases). No 2FA, no app lock on any of them.
   - Reviewer: `review@coflop.fr`, first name `Review`, a strong password used nowhere else.
   - Demo friends: `review2@coflop.fr` "Léa", `review3@coflop.fr` "Hugo", `review4@coflop.fr` "Inès". Four people in all: enough for the global question and a ranking.
2. **Room**: as `review@`, create "Play Review", language **English** (reviewers most likely read English; questions come from the room's language), vibe Entre potes / Friends. The reviewer owns it, so they can see the owner tools (room settings, remove a member, delete or restore messages). Leave pacing off, Tripot on, anonymous votes "Au choix" (the defaults). Léa, Hugo and Inès join with the room code.
3. **Every day until you submit**: each of the four votes at least once (streak flames next to names and the room streak; the first vote of the day also gives +20 chips).
4. **Play 6 to 8 questions** with all four, among them:
   - a few built-in "Which of us" questions;
   - a **ranking** ("Rank us…": write one in the composer with the Classement / Ranking type if the deck doesn't draw one);
   - a **choose** question ("Who gets what", 2 to 4 answers, e.g. "Who orders what? | Pizza | Sushi | Burger");
   - an **anonymous** question: composer switch "Anonyme" / "Anonymous";
   - one where Léa and Hugo **bet** ("Parier" / "Place a bet") before it closes, so a result shows a pot and winners.
5. **Chat** under two or three results, from the demo accounts: a few messages, one harmless photo (a pizza, a view), one GIF, a few reactions, a mention of Review. Pin one result in History.
6. **Questions tab**: Léa and Hugo each add a custom question that stays in the queue (something for "Report this question" and drag-to-reorder). Install one public pack: Questions > Packs > a pack > "Installer" / "Add".
7. **Final state, the day you submit**: a new built-in "Which of us" question is open (not anonymous); Léa, Hugo and Inès have voted on it and Léa and Hugo have bet on it; Review hasn't voted on it, hasn't bet, and hasn't answered today's global question. The reviewer's vote then reveals a result right away. Sign out of `review@` everywhere you used it.
8. **Before every later submission**, check the room again: a reviewer's vote opens the next question with nobody's votes in, and a question open 24 h closes by itself once half the room voted (late quorum, docs/ROOMS.md), so redo step 7. Make sure the reviewer is still the owner and nothing was deleted.

In the Console (App content > App access > Add instructions): name `Reviewer`, username `review@coflop.fr`, password the one you chose, and in "Any other instructions" the text below. The Console's limit for that field isn't documented (we couldn't confirm it); the short version is 480 characters (`wc -m`) and fits a 500 limit. If the field takes more, paste the long version instead.

Short version:

```
Open app > Continue with email > enter e-mail > Sign in. No 2FA or code. You own room "Play Review" (3 test friends). Bottom tabs: Vote, History, Stats, Questions. Vote: tap a face > Vote for. Place a bet = Tripot, free fake chips, no money. Results + chat (photos/GIFs) under each result. Packs: Questions > Packs. Party: tap room name > Start a party. Report/block: long-press a message; people icon > ... by a member. Delete account: avatar > Security. Please don't delete it.
```

Long version (1832 characters):

```
SIGN IN
1. Open the app > "Continue with email" > review@coflop.fr > Continue > enter the password > "Sign in". No 2FA, no e-mail code. On a French phone: "Continuer avec un e-mail", then "Se connecter".
2. You land in the room "Play Review". You are its owner; Léa, Hugo and Inès are our test accounts. The app follows the phone language (English unless the phone is in French); Profile > Language changes it.

PLAY
- Vote (1st icon at the bottom): an open question. Tap a face, then "Vote for ...". The others have voted, so your vote shows the result.
- "Place a bet" on the question is Le Tripot: bets with fictional chips (100 a month for free; nothing to buy, no cash value, no prizes).
- "Global question" card under the header: same question in every room; other rooms only appear as anonymous percentages.
- Results show who voted for whom (only totals on questions marked "Anonymous"). Chat under each result: text, photos and GIFs (image+ button by the message field), reactions (long-press).
- History (2nd icon): past results; tap one for its result and chat. Stats (3rd): awards, streaks, Tripot board. Questions (4th): upcoming questions, write your own, "Packs" (community packs), "Suggest".
- Party mode: tap the room name (top left) > "Start a party". It needs 2 players: open the party link in a browser and join as a guest, no account needed.

SAFETY
- Report or block: long-press a chat message from someone else; long-press a question someone else wrote (Questions tab); people icon (top right) > "..." next to a member; a pack's page > "Report".
- Owner tools: people icon > gear = Room settings (anonymous votes, Tripot on/off); "..." next to a member to remove them.
- Delete account: your avatar (top right) > Security > "Delete my account". Please don't delete this account: we reuse it for every review.
```

UI labels used above, French / English (`app/src/locales/{fr,en}`): Continuer avec un e-mail / Continue with email; Continuer / Continue; Se connecter / Sign in; Voter pour … / Vote for …; Parier / Place a bet; Question du monde / Global question; Anonyme / Anonymous; Historique / History; Packs / Packs; Proposer / Suggest; Lancer une soirée / Start a party; Signaler ce message / Report this message; Bloquer … / Block …; Signaler / Report (pack); Paramètres du salon / Room settings; Profil / Profile; Langue / Language; Sécurité / Security; Supprimer mon compte / Delete my account.

## Closed testing (required before production)

New personal developer accounts must run a **closed test with at least 12 testers opted in for 14 days in a row** before they can apply for production access. Testers who opt out, or uninstall, can break the streak; aim for 15 to 20 to keep a margin.

Setup:

1. Testing > Closed testing > Create track (or use "Alpha"). Upload the AAB, release notes in French, roll out.
2. Testers: either a **Google Group** (create `quidenous-testers@googlegroups.com`, add people, add the group to the track; easiest to manage) or an **e-mail list** in the Console (their Google account e-mails).
3. Countries: France (plus wherever testers live).
4. Copy the **opt-in link** from the track page and send it. Each tester: opens the link on their Android phone with the right Google account > "Devenir testeur" > installs from the Play Store link > signs in > keeps the app installed and opens it now and then for 14 days.
5. Ask them to use "Envoyer des commentaires" on the Play page or write to contact@coflop.fr; Google asks you about tester engagement and feedback when you apply.

Timeline:

- [ ] Day 0: store listing, content rating, target audience, data safety, app access filled in (the Console blocks rollout until done)
- [ ] Day 0: closed test release rolled out, opt-in link sent (review of a first release can take a few days)
- [ ] Day 1-2: check 12+ testers show as opted in (Console > track > Testers)
- [ ] Day 2-14: ship at least one update to the closed track (shows active development), collect feedback
- [ ] Day 14+ (counted from the day the 12th tester opted in): Dashboard > "Apply for production", answer the questions about the test (how testers were recruited, feedback, changes made)
- [ ] After approval (usually up to 7 days): create the production release

## Release checklist

Done in the repo (checked at v0.39.0, dropped from the list): real launcher icon, adaptive icon and splash (lime `?` on #0E0E12, `app/app.json`); privacy and terms say 16+; terms say "une question à la fois"; the privacy page's "votes selon les règles du salon" matches anonymous votes; `expo.version` 0.39.0 and `android.versionCode` 65 in `app/app.json` (the `app` workflow fails a tag whose version doesn't match or whose versionCode doesn't go up).

- [ ] Privacy policy (FR and EN) updated for chat photos, Klipy, time-zone offset, Tripot and the global question (docs/PLAY_DATA_SAFETY.md, "Policy-side items still open"), plus the controller's legal name and address
- [ ] Optional: soften the deck questions listed under Content rating (Nigerian prince in both languages, "coucher … promotion" and "torture" in French)
- [ ] Production runs v0.39.0 with `AUTH_REQUIRED=1`, `REPORT_EMAIL`, `ADMIN_USER_IDS`, Mailgun (codes, verification, reset, notices), `KLIPY_API_KEY` (or drop "GIF" from the description), Google and Discord OAuth (or drop the missing one from the description), `CLIENT_ERRORS_WEBHOOK`; `/delete-account`, `/privacy`, `/terms` live. `GET https://qdn.coflop.fr/api/features` should list `tripot`, `streaks`, `anonymous_votes`, `global_question`, `chat_images`, `chat_gifs`, `party`, `oauth_google`, `oauth_discord`, `email`
- [ ] AAB: GitHub Actions workflow **`android`**, job `apk`, built on every `v*` tag (and on pushes to `main` touching `app/`). Take the **`quidenous-aab`** artifact (`app-release.aab`, signed with the upload key from the `ANDROID_UPLOAD_KEYSTORE_B64` / `ANDROID_UPLOAD_KEYSTORE_PASSWORD` secrets) from the **v0.39.0 tag run**: production and the OTA updates follow tags, not `main`. If the artifact is missing, the upload key secrets are not set (the step logs "no upload key, skipping"). Upload once per versionCode: 65 can't be reused after an upload
- [ ] Play App Signing: on the first upload accept "Google manages your app signing key". The upload key already exists; register it if the Console asks (Setup > App integrity > upload key certificate). The sideload APK (`quidenous-apk`) keeps its old key: Play installs can't update hand-installed APKs and vice versa, testers must uninstall the APK first
- [ ] Content rating questionnaire answered with the v0.39.0 answers above (simulated gambling **Yes**, alcohol references **Yes**); target audience 16-17 and 18+; Advertising ID declared
- [ ] Countries: decide whether to include Australia (R 18+) and Brazil (likely 18) because of the Tripot
- [ ] Data safety form filled from docs/PLAY_DATA_SAFETY.md
- [ ] Reviewer account and demo room created on production (see App access), state checked the day you submit
- [ ] Upload the AAB to the closed testing track, release notes (FR), roll out
- [ ] Listing assets uploaded: icon, feature graphic, the new phone screenshots, short and full descriptions
