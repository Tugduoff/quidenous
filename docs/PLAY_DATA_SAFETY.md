# Play Console: Data safety form (draft)

What to answer in Play Console > App content > Data safety, from the code as of migration `0034_privacy.sql` and
`server/assets/legal/privacy.html`. Re-check it whenever a migration adds personal data, a
processor is added, or the privacy policy changes. The Play build has no self-update (no GitHub calls, no
`REQUEST_INSTALL_PACKAGES`, `app.config.ts`), no analytics, ads or crash SDK.

## Overview

| Question | Answer |
| --- | --- |
| Does your app collect or share any of the required user data types? | **Yes** |
| Is all of the user data collected by your app encrypted in transit? | **Yes** (HTTPS only, `https://qdn.coflop.fr`) |
| Do you provide a way for users to request that their data is deleted? | **Yes**: in the app (Profil > Sécurité > Supprimer mon compte) and on the web at https://qdn.coflop.fr/delete-account |
| Account creation methods | Username and password (e-mail), OAuth (Google, Discord), and "other": a first-name account with no credentials (legacy join, party guest) |
| Delete account URL | https://qdn.coflop.fr/delete-account |
| Data shared with third parties | **None** |

Not "sharing" under Play's definition (service providers processing on our behalf, or transfers the user starts):
- Mailgun: sends verification, reset and deletion codes to the e-mail address.
- Google and Discord sign-in: they send us an account ID, name and verified e-mail; we send them nothing.
- Firebase Cloud Messaging and browser push services: deliver notifications (push token and notification text).
- Discord webhook (`CLIENT_ERRORS_WEBHOOK`, optional): the team's private channel receives app crash reports.
- Public share links (`/s/…`) and public packs (`/p/…`): published by the user on purpose (user-initiated), first names
  and drawn avatars only, 90-day expiry, revocable, `noindex`.

## Data types

Answer "Not collected" for everything not listed (approximate or precise location, financial info, health and
fitness, contacts, calendar, audio, files and docs, web browsing, search history, installed apps, diagnostics, other
performance data, race, religion, political or sexual orientation, phone number, address).

| Play data type | Collected | Shared | Ephemeral | Required / optional | Purposes | What it is |
| --- | --- | --- | --- | --- | --- | --- |
| Personal info > **Name** | Yes | No | No | Required | App functionality, Account management | First name shown to room members, on share cards (first name only), packs and suggestions (also from Google / Discord when used). |
| Personal info > **Email address** | Yes | No | No | Optional | Account management, Fraud prevention, security and compliance | Sign-in, verification codes, password reset, deletion code. First-name and party guest accounts have none. |
| Personal info > **User IDs** | Yes | No | No | Required | App functionality, Account management | Internal account ID, friend code, Google / Discord account ID when linked. |
| Personal info > **Other info** | Yes | No | No | Required | Account management, Fraud prevention, security and compliance | Age confirmation (16+) and terms acceptance with its date and version (`users.terms_version`, `terms_at`); 16+ content opt-in. |
| Photos and videos > **Photos** | Yes | No | No | Optional | App functionality | Profile photo if added (re-encoded to 256 px, EXIF dropped). The camera only scans QR codes on the device. |
| Messages > **Other in-app messages** | Yes | No | No | Optional | App functionality | Chat under results (room members only). Deleted with the account. |
| App activity > **App interactions** | Yes | No | No | Required | App functionality | Votes, rankings, choices, nudges, skips, pins, reactions, likes, installs, with timestamps (results, history, stats). |
| App activity > **Other user-generated content** | Yes | No | No | Optional | App functionality | Questions added to rooms, published packs, deck suggestions, share links, reports (reason, details, snapshot of the reported content). |
| App info and performance > **Crash logs** | Yes | No | No | Required (automatic) | App functionality (fixing crashes) | When a screen crashes: error message and stack, screen path, app version, platform, account ID. Logged on the server and optionally posted to the team's private Discord channel. |
| Device or other IDs | Yes | No | No | Optional | App functionality, Account management | Push token (FCM) when notifications are allowed; device label (e.g. "Android · Pixel 8") in the sessions list. |

Covered by the rows above, not separate types: sessions (tokens stored hashed), passwords (hashed), 2FA secrets
(encrypted), blocks and friendships. IP addresses are only kept in memory for rate limiting (not stored, not a
declared type).

## Retention and deletion (if asked, and to keep the policy honest)

| Data | Kept | Code |
| --- | --- | --- |
| Account, profile, game content | Until the account is deleted | `accounts/delete.rs` `erase()` |
| Party guest accounts | Erased 30 days after their last party ends, unless secured | `guests.rs` (`GUEST_TTL_DAYS`, hourly purge) |
| Sessions | Until sign-out, revocation or deletion; unused for 180 days: ended (accounts that can sign in again) | `accounts/mod.rs` (`IDLE_DAYS`) |
| E-mail codes | 15-30 min, rows purged a day after expiry | `accounts/mod.rs` `codes` |
| Share links | 90 days for new ones, revocable (`DELETE /api/shares/{token}`) | `share.rs` |
| Reports | No automatic purge; the reported member's name is scrubbed when they delete their account | `moderation.rs`, `erase()` |
| Soft-deleted rooms | Kept in the database (not reachable) | `rooms.rs` |
| Backups | 30 days off-host, 3 local snapshots | `deploy/backup.sh` |
| Docker logs | 3 × 10 MB, rotated | `deploy/compose.yaml` |

What deletion does: profile, e-mail, password, 2FA, sessions, linked providers, push tokens, friends, friend requests,
room invitations, blocks, notification settings, chat messages, inbox entries, share links the user made and stat
cards showing them are deleted; reports about them as a member lose their name; seats are anonymised ("Compte
supprimé"); votes, questions and packs stay anonymised so the others' history adds up. The privacy policy says so.

## Policy-side items still open

- The privacy policy names the controller as "coflop" with contact@coflop.fr: add the legal name and postal address.
- Mailgun region: the policy covers both EU and US (DPF + SCCs). Prefer `MAILGUN_REGION=eu` with an EU domain.
- No automatic purge of reports, soft-deleted rooms or inactive accounts yet: if one is added, update the policy and
  the table above.
