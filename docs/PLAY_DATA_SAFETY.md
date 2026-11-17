# Play Console: Data safety form (draft)

What to answer in Play Console > App content > Data safety, from the code as of v0.39.0 (migration `0047_tripot.sql`)
and `server/assets/legal/privacy.html`. Re-check it whenever a migration adds personal data, a
processor is added, or the privacy policy changes. The Play build has no self-update (no GitHub calls, no
`REQUEST_INSTALL_PACKAGES`, `app.config.ts`), no analytics, ads or crash SDK. JS updates come from our own server
(docs/OTA.md), which sends nothing about the user.

## Overview

| Question | Answer |
| --- | --- |
| Does your app collect or share any of the required user data types? | **Yes** |
| Is all of the user data collected by your app encrypted in transit? | **Yes** (HTTPS only, `https://qdn.coflop.fr`) |
| Do you provide a way for users to request that their data is deleted? | **Yes**: in the app (Profil > Sécurité > Supprimer mon compte) and on the web at https://qdn.coflop.fr/delete-account |
| Can users ask for some data to be deleted without deleting the account? (optional) | **Yes**: they delete their own chat messages and photos, questions, share links (Profil > Mes liens), profile photo, packs; anything else by e-mail to contact@coflop.fr |
| Account creation methods | Username and password (e-mail), OAuth (Google, Discord), and "Other": party guest (first name only, no credentials, erased 30 days after its last party). Production runs with `AUTH_REQUIRED=1`, so the old first-name join (`/api/join`, `/api/start`) can't create accounts any more; existing legacy accounts are still there |
| Delete account URL | https://qdn.coflop.fr/delete-account |
| Data shared with third parties | **None** (see below) |

Not "sharing" under Play's definition (a service provider processing on our behalf, or a transfer the user starts
and expects):
- Mailgun: sends verification, reset, deletion codes and security notices to the e-mail address.
- Google and Discord sign-in: they send us an account ID, name and verified e-mail; we send them nothing.
- Firebase Cloud Messaging and browser push services: deliver notifications (push token and notification text).
- Klipy (GIF search, only when `KLIPY_API_KEY` is set): the server, not the phone, sends Klipy the search text, the
  region picked from the app language (`FR` / `GB`), a content-filter level and an opaque per-account ID (an HMAC,
  not the account ID); no IP address, name or e-mail. GIF files are fetched and streamed back by our server, never
  stored. The user starts it (the field says "Rechercher sur KLIPY"), and Klipy only answers the request: service
  provider + user-initiated. If Klipy's API terms let it reuse queries for its own purposes (ads, profiling), declare
  "In-app search history" as **shared** instead.
- Discord webhook (`CLIENT_ERRORS_WEBHOOK`, optional): the team's private channel receives app error reports.
- Public share links (`/s/…`) and public packs (`/p/…`): published by the user on purpose (user-initiated), first names
  and drawn avatars only, 90-day expiry, revocable, `noindex`. The global question of the day only shows other
  rooms as percentages (at least 5 rooms), never a name, room or member.

## Data types

Answer "Not collected" for everything not listed (approximate or precise location, financial info, health and
fitness, contacts, calendar, audio, videos, files and docs, web browsing, installed apps, diagnostics, other
performance data, race, religion, political or sexual orientation, phone number, address).

| Play data type | Collected | Shared | Ephemeral | Required / optional | Purposes | What it is |
| --- | --- | --- | --- | --- | --- | --- |
| Personal info > **Name** | Yes | No | No | Required | App functionality, Account management | First name shown to room members (party guests included), on share cards (first name only), packs and suggestions (also from Google / Discord when used). |
| Personal info > **Email address** | Yes | No | No | Optional | Account management, Fraud prevention, security and compliance | Sign-in, verification codes, password reset, deletion code, security notices. Party guest and legacy accounts have none. |
| Personal info > **User IDs** | Yes | No | No | Required | App functionality, Account management | Internal account ID, friend code, Google / Discord account ID when linked. |
| Personal info > **Other info** | Yes | No | No | Required | App functionality, Account management, Fraud prevention, security and compliance | Age confirmation (16+) and terms acceptance with its date and version (`users.terms_version`, `terms_at`); 16+ content opt-in; app language; UTC offset of the phone's time zone (`notification_prefs.tz_offset`: quiet hours and streak days; an offset, not a location). |
| Photos and videos > **Photos** | Yes | No | No | Optional | App functionality | Profile photo (re-encoded to 256 px) and photos sent in the chat (JPEG / WebP re-encoded, at most 1600 px; PNG re-encoded; GIF kept as is, it has no EXIF): metadata such as location is dropped. Chat photos are stored on our server, shown to the room's members through signed links that expire, and deleted with their message, their author's account or the room. The camera scans QR codes and takes chat photos. |
| Messages > **Other in-app messages** | Yes | No | No | Optional | App functionality | Chat under results (room members only): text, captions, mentions, reactions, the GIFs picked (Klipy ID and URL). Deleted with the account. |
| App activity > **App interactions** | Yes | No | No | Required | App functionality | Votes, rankings, choices, anonymous-question flags, the global question vote, Tripot bets and chip ledger (fictional chips), nudges, skips, pins, reactions, likes, installs, with timestamps (results, history, stats, streaks). |
| App activity > **In-app search history** | Yes | No | **Yes** | Optional | App functionality | GIF search text (forwarded to Klipy, see above) and pack search text: used for that one request, never stored or logged. |
| App activity > **Other user-generated content** | Yes | No | No | Optional | App functionality | Questions added to rooms, published packs, deck suggestions, share links, reports (reason, details, snapshot of the reported content). |
| App info and performance > **Crash logs** | Yes | No | No | Required (automatic) | Analytics | When the app hits an unexpected error (a screen crash, an uncaught error or an unhandled promise rejection; at most 5 per session): error message and stack, app version, platform, screen path (web only), account ID. Logged on the server and optionally posted to the team's private Discord channel. Play's "Analytics" purpose is the one that covers monitoring app health and fixing crashes. |
| Device or other IDs | Yes | No | No | Optional | App functionality, Account management | Push token (FCM) when notifications are allowed; device label (e.g. "Android · Pixel 8") in the sessions list. |

Covered by the rows above, not separate types: sessions (tokens stored hashed), passwords (hashed), 2FA secrets
(encrypted), blocks and friendships, streaks (computed from votes, only the last evening reminder is stored). IP
addresses are only kept in memory for rate limiting (not stored, not a declared type).

## Retention and deletion (if asked, and to keep the policy honest)

| Data | Kept | Code |
| --- | --- | --- |
| Account, profile, game content | Until the account is deleted | `accounts/delete.rs` `erase()` |
| Chat photos | Until their message is deleted (author, owner, admin), their author's account is deleted or the room is deleted; in the nightly backup | `media.rs`, `erase()` |
| GIF and pack searches | Not stored (in memory for the request) | `gifs.rs`, `packs.rs` |
| Tripot ledger and bets, global question votes | Like game content; the seat is anonymised when the account is deleted | `tripot.rs`, `global.rs` |
| Party guest accounts | Erased 30 days after their last party ends, unless secured | `guests.rs` (`GUEST_TTL_DAYS`, hourly purge) |
| Sessions | Until sign-out, revocation or deletion; unused for 180 days: ended (accounts that can sign in again) | `accounts/mod.rs` (`IDLE_DAYS`) |
| E-mail codes | 15-30 min, rows purged a day after expiry | `accounts/mod.rs` `codes` |
| Share links | 90 days for new ones, revocable (`DELETE /api/shares/{token}`) | `share.rs` |
| Reports | No automatic purge; the reported member's name is scrubbed when they delete their account | `moderation.rs`, `erase()` |
| Soft-deleted rooms | Kept in the database (not reachable); their chat photos are deleted | `rooms.rs` |
| Crash reports | In the server log (rotated, below) and the Discord channel (no automatic purge there) | `ops.rs` |
| Backups | 30 days off-host, 3 local snapshots | `deploy/backup.sh` |
| Docker logs | 3 × 10 MB, rotated | `deploy/compose.yaml` |

What deletion does: profile, e-mail, password, 2FA, sessions, linked providers, push tokens, friends, friend requests,
room invitations, blocks, notification settings, chat messages with their photos and reactions, inbox entries, share
links the user made and stat cards showing them are deleted; reports about them as a member lose their name; seats
are anonymised ("Compte supprimé"); votes, bets, questions and packs stay anonymised so the others' history adds up.
The privacy policy says so.

## Policy-side items still open

- **The privacy policy is behind the app** (`server/assets/legal/privacy.html` and `privacy.en.html`, both still
  "version 1" of 16 November 2026): it doesn't mention chat photos (stored on our server), GIF search through Klipy
  (a provider to add to "Prestataires"), the time-zone offset, the Tripot (fictional chips) or the global question
  (anonymous aggregates across rooms). Play checks that the policy and this form agree: update both pages before
  submitting. A wording-only update doesn't need a `TERMS_VERSION` bump.
- The privacy policy names the controller as "coflop" with contact@coflop.fr: add the legal name and postal address.
- Mailgun region: the policy covers both EU and US (DPF + SCCs). Prefer `MAILGUN_REGION=eu` with an EU domain.
- No automatic purge of reports, soft-deleted rooms or inactive accounts yet: if one is added, update the policy and
  the table above.
