# Google Play listing: qui de nous ?

Everything to copy into the Play Console. Package `com.tugduoff.quidenous`. Nothing in this folder ships in the app.

## Files

| File | What | Size |
| --- | --- | --- |
| `short-description.txt` | Short description (FR), 71 / 80 chars | 73 B |
| `full-description.txt` | Full description (FR), 2654 / 4000 chars | 2.7 KB |
| `icon-512.png` | App icon, 512x512, 32-bit PNG, full-bleed square (Play applies the mask) | 88 KB |
| `feature-graphic.png` | Feature graphic, 1024x500, 24-bit PNG, no alpha | 141 KB |
| `screenshots/01-vote.png` | Phone, 1080x1920: voting on a question | 134 KB |
| `screenshots/02-result.png` | Phone, 1080x1920: result + chat | 146 KB |
| `screenshots/03-ranking.png` | Phone, 1080x1920: "Classe-nous" ranking result | 130 KB |
| `screenshots/04-history.png` | Phone, 1080x1920: history | 142 KB |
| `screenshots/05-questions.png` | Phone, 1080x1920: the room's own questions | 143 KB |
| `screenshots/06-stats.png` | Phone, 1080x1920: stats and awards | 138 KB |

Screenshots are the real web build (same code as the app) in the dark theme, with a demo room (Léa, Hugo, Inès, Nathan, Camille, Jules). No real user data.

### Launcher icon

The app shipped the stock Expo icon (blue chevron). This branch replaces it with the store icon (lime `?` on #0E0E12, Bricolage Grotesque, like the in-app logo): `app/assets/images/icon.png`, the Android adaptive icon (foreground, background, monochrome), favicon, splash (now #0E0E12) and `app/public/icon-*.png`. It needs a new native build to show on phones.

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

Wording to check before publishing:

- The description says "une question à la fois ... une par jour ou dix d'affilée" because the server opens the next question as soon as everyone has voted (no daily timer). The terms page says "une question par jour": align one or the other.
- Votes: secret while the question is open, then the result shows who voted for whom, always (no room setting for anonymous votes). The privacy page says "votes selon les règles du salon": fine if that means this, but there is no per-room option.

## Content rating (IARC questionnaire)

Category: **Game** (or "Social networking / UGC" if you list it as Social; the answers are the same).

| Section | Answer | Why |
| --- | --- | --- |
| Violence (realistic, fantasy, blood, gore) | **No** | Text only. Some built-in questions mention prison, zombies, a serial killer, torture, as jokes; no depiction. |
| Fear / horror | **No** | |
| Sexuality: nudity | **No** | |
| Sexuality: sexual innuendo / references | **Yes, mild, text only** | Built-in questions such as "Qui pourrait coucher avec quelqu'un pour une promotion ?", "proposition indécente", cheating. |
| Language (profanity, crude) | **Yes, mild** | "connerie" in a built-in question; user chat is unfiltered (covered by the interaction questions). |
| Crude humour | **Yes** | The whole game is teasing friends. |
| Controlled substances (drugs, alcohol, tobacco) | **No** | No references in the built-in questions (users may write anything). |
| Gambling: real money / simulated gambling | **No / No** | Only text mentions (casino, loto, crypto). No betting, no currency. |
| Discrimination / hate | **No** | (See the note below on one question.) |
| Users can interact or communicate (chat, messaging) | **Yes** | Chat under every result, inside private rooms only. |
| User-generated content shared with other users | **Yes** | Custom questions and chat (room members only), public question packs (all users), profile name and photo. |
| Moderation tools | **Yes** | Report a message, question, member or pack; block a user; room owner can remove members; members can vote someone out. |
| Shares the user's location | **No** | |
| Digital purchases / loot boxes | **No** | |
| Unrestricted internet access / web browser | **No** | The browser only opens for Google/Discord sign-in. |
| Is the app a web browser or search engine | **No** | |

Expected result: PEGI 12 to 16, USK 12 to 16, ESRB Teen to Mature 17+, plus the "Users Interact" notice. The computed rating can be lower than the 16+ target audience: that's fine, the target audience setting below is what keeps it away from kids.

Built-in questions worth softening in `server/deck/fr.txt` (they drive the rating and could draw a policy flag): "Qui se ferait arnaquer par un faux prince nigérian ?" (national stereotype), "Qui pourrait coucher avec quelqu'un pour une promotion ?", "Qui serait le premier à dire oui à une proposition indécente ?", "Qui serait le premier à céder sous la torture ?".

## Target audience and content

| Question | Answer |
| --- | --- |
| Target age groups | **16-17 and 18+** only |
| Appeals to children? | No |
| Designed for children / Families program | No |
| Contains ads | **No** |
| In-app purchases | **No** (free, nothing to buy) |
| News app | No |
| COVID-19 contact tracing | No |
| Government app | No |
| Financial features | None |
| Health features | None |

The privacy page says **16 ans et plus** (changed from 15 in this branch).

## Data safety

Based on `server/migrations/*.sql`, `server/assets/legal/privacy.html` and the account deletion / reports / blocks contract. No analytics, no ads SDK, no crash reporting SDK.

### Overview

| Question | Answer |
| --- | --- |
| Does your app collect or share any of the required user data types? | **Yes** |
| Is all user data encrypted in transit? | **Yes** (HTTPS only: `https://qdn.coflop.fr`) |
| Do you provide a way for users to request that their data is deleted? | **Yes**: in the app (Profil > Sécurité > Supprimer mon compte) and on the web at https://qdn.coflop.fr/delete-account |
| Data shared with third parties | **None** |

Not "sharing" under Play's definition (transfer to a service provider that processes data on our behalf): Mailgun (sends verification / reset / deletion codes to the e-mail address), Google and Discord sign-in (they send us an ID, name and verified e-mail; we send them nothing), Firebase Cloud Messaging and browser push services (deliver notifications, receive the push token and the notification text). None of them get data for their own use, and there is no advertising.

### Data types

Answer "Not collected" for everything not listed (location, financial, health, contacts, calendar, audio, files, web browsing, search history, installed apps, crash logs, diagnostics, other performance data, etc.).

| Play data type | Collected | Shared | Processed ephemerally | Required or optional | Purposes | What it is |
| --- | --- | --- | --- | --- | --- | --- |
| Personal info > **Name** | Yes | No | No | Required | App functionality, Account management | First name / display name shown to room members (also from Google/Discord when used). |
| Personal info > **Email address** | Yes | No | No | Required | Account management, Fraud prevention, security and compliance | Sign-in, verification codes, password reset, account deletion code. (Old accounts created before sign-in existed may have none.) |
| Personal info > **User IDs** | Yes | No | No | Required | App functionality, Account management | Internal account ID, friend code, Google/Discord account ID when linked. |
| Photos and videos > **Photos** | Yes | No | No | Optional | App functionality | Profile photo if the user adds one (resized to 256 px). The camera is only used to scan QR codes, on the device, nothing is uploaded. |
| Messages > **Other in-app messages** | Yes | No | No | Optional | App functionality | Chat messages under results (room members only). Deleted with the account. |
| App activity > **App interactions** | Yes | No | No | Required | App functionality | Votes, rankings, reminders sent, skips, pins, reactions, with timestamps (used for results, history and room stats). |
| App activity > **Other user-generated content** | Yes | No | No | Optional | App functionality | Questions added to a room (typed or imported from a text file), published question packs, reports (reason, details, snapshot of the reported content). |
| Device or other IDs | Yes | No | No | Optional | App functionality, Account management | Push notification token (FCM) when notifications are allowed; device label ("Android · Pixel 8") in the sessions list. |

Security sessions (login tokens, stored hashed), passwords (hashed), 2FA secrets (encrypted) are account data covered by the rows above. Short-lived server logs and in-memory rate limiting by IP are not a declared data type.

What deletion does (to describe if asked): profile, e-mail, password, 2FA, sessions, linked providers, push tokens, friends, blocks and chat messages are deleted; the user's seat is anonymised in rooms; votes, questions and packs they created stay, anonymised, so other members' history still adds up. Privacy policy states this ("l'historique des salons garde tes votes, anonymisés").

## App access (for reviewers)

All features are behind sign-in, so declare **"All or some functionality is restricted"** and give a login. Create it on production **after** deploying the release (not done yet, nothing was created on production):

1. In the app (or https://qdn.coflop.fr), "Continuer avec un e-mail": `review@coflop.fr`, a strong password you choose, first name `Review`. Do **not** turn on 2FA for it (reviewers can't get codes). Verify the e-mail so nothing nags.
2. Create a room "Démo Play".
3. Make two or three more accounts (e.g. `review2@coflop.fr`, `review3@coflop.fr`, names Léa / Hugo), join the room with its code, play 3 to 5 questions with all accounts so the history, stats and a chat exist. Leave the current question with the other accounts' votes in and `review@` not voted, so the reviewer lands on a vote.
4. Add one or two custom questions from another account so the Questions tab and the report action have something to show.

Paste into the Console (Instructions field):

```
Sign in: open the app > "Continuer avec un e-mail" > review@coflop.fr > password: <PASSWORD>
The account is in the room "Démo Play" with other test accounts.
- Vote tab: tap a person, then "Voter pour …". When everyone has voted the result shows.
- Results show who voted for whom; chat is under each result.
- History, Stats and Questions tabs at the bottom. Packs: Questions > Packs.
- Report / block: long-press a chat message or a question written by someone else; on the room page (people icon at the top) tap "..." next to a member.
- Account deletion: Profil (avatar top right) > Sécurité > Supprimer mon compte. Please don't delete this account.
No 2FA, no e-mail code needed.
```

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

- [ ] Replace the placeholder launcher icons and splash (see above), bump `expo.version` and `android.versionCode` in `app/app.json`
- [ ] Privacy page: age 16+, votes wording; terms: "une question par jour" wording
- [ ] Server deployed with account deletion (`/delete-account` page live), reports and blocks, `REPORT_EMAIL` set, Mailgun configured (deletion codes, verification, reset)
- [ ] Push on main → GitHub Actions `android` workflow → download the **`quidenous-aab`** artifact (`app-release.aab`, signed with the upload key from the `ANDROID_UPLOAD_KEYSTORE_B64` secret). If the artifact is missing, the upload key secrets are not set (the step logs "no upload key, skipping").
- [ ] Play App Signing: on the first upload accept "Google manages your app signing key". The upload key already exists; register it if the Console asks (Setup > App integrity > upload key certificate). The sideload APK (`quidenous-apk`) keeps its old key: Play installs can't update hand-installed APKs and vice versa, testers must uninstall the APK first.
- [ ] Upload the AAB to the closed testing track, release notes (FR), roll out
- [ ] Reviewer account created on production (see App access)
- [ ] Listing assets uploaded: icon, feature graphic, 6 phone screenshots, descriptions
