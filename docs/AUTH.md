# Accounts, sign-in, 2FA, sessions

Status: design + implementation, 2026-11-12 (branch `feat/auth`). Goal: open the app to the public without "open the app = you are someone". The live group must never be signed out.

## Model

An account is still a `users` row and a session is still a bearer token in `user_tokens` (works the same for the phone app and the web app). What is new is *how* you get a token.

Migration `0015_auth.sql`:

| Table / column | |
| --- | --- |
| `users.email` | lowercase, trimmed, unique when set (partial unique index); `NULL` for legacy accounts |
| `users.password_hash` | argon2id PHC string, `NULL` if the account has no password (legacy or OAuth only) |
| `users.email_verified_at` | set by the 6-digit code, a password reset, or a provider that vouches for the email |
| `users.totp_secret` | AES-256-GCM encrypted (key from `AUTH_SECRET`), `NULL` = 2FA off |
| `users.totp_pending` | encrypted secret between "set up" and "enable" |
| `users.totp_last_step` | last accepted 30 s step: a code can't be replayed |
| `recovery_codes` | `user_id, code_hash, used_at`: 10 one-time codes, keyed hash (HMAC-SHA256 with a key from `AUTH_SECRET`) |
| `oauth_identities` | `user_id, provider ('google'/'discord'), subject, email`, unique `(provider, subject)`; `provider` is free text so another one (Apple) can be added later |
| `auth_codes` | short-lived secrets, hashed: `kind` = `verify_email` / `reset_password` / `mfa` / `oauth_state` / `oauth_result` / `delete_account` (0016), `user_id`, `email`, `data` (JSON), `expires_at`, `used_at`, `attempts` |
| `user_tokens.last_used_at`, `.device` | session metadata (device label from `X-Device`, else the User-Agent, 80 chars) |
| `user_tokens.claimed` | `1` = handed over by `/api/join` by first name (see *Legacy name grab*) |

A session's public id is the first 16 hex chars of its token hash (the token itself is never stored or shown).

Account kinds: **secured** = has an email + password, or at least one linked provider. **Legacy** = token only (everyone in the live group today).

## Security choices

- **Passwords**: argon2id, m = 19 MiB, t = 2, p = 1 (OWASP minimum, ~50 ms on a Pi-class CPU), hashed on a blocking thread, at most 2 at once. 8 to 128 characters, rejected if in a built-in list of the most common passwords or equal to the email. Unknown email and wrong password give the same `bad_credentials`, and an unknown email still pays for one hash (no timing oracle).
- **Codes** (email verification, password reset): 6 digits, 15 min (verify) / 30 min (reset), single use, 5 wrong tries kill the code, stored as a keyed hash. A new code replaces the previous one. Every guess (codes, 2FA challenges, OAuth results) uses up its try in one `UPDATE … WHERE attempts < 5` *before* it is compared, and marking a code used is one `UPDATE … WHERE used_at IS NULL`: parallel guesses can't share a try, a code can't be redeemed twice.
- **2FA**: TOTP RFC 6238 (SHA-1, 6 digits, 30 s, ±1 step, no replay). Enabling needs a valid code from the app. 10 recovery codes `xxxxx-xxxxx`, one-time, shown once, can be regenerated. Sign-in with 2FA: the first step returns a `challenge` (5 min, 5 tries), the second step trades it plus a code for the session.
- **Rate limits** (in memory, token buckets, per IP and per account): see below. `429 {"error":"rate_limited","retry_after":s}`.
- **Client IP**: the socket address; with `TRUST_PROXY=1` the **last** `X-Forwarded-For` entry, the one the (single) reverse proxy appended: the entries before it are whatever the client sent. Set it behind the homelab reverse proxy (which must *append* to the header: Caddy and Traefik do by default, nginx needs `proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;`), never when the port is exposed directly. Without it behind a proxy every client shares the proxy's IP, so the anonymous `/join` and `/start` limits throttle everyone at once.
- **Sessions**: a reset password signs out every device. If the email had never been verified, the reset is the first proof of who owns it: it also removes the linked providers, 2FA (secret, pending set-up, recovery codes) that whoever typed the address may have added, and that 2FA doesn't apply to the reset's own sign-in. With an already verified email, 2FA still applies (challenge). Changing the password and enabling 2FA offer to sign out the others (on by default in the app). Sign-out deletes the token server-side.
- **OAuth**: authorization code + PKCE with the provider (done by the server, which holds the client secrets), `state` and `nonce` checked, Google ID tokens verified against Google's JWKS (signature, `iss`, `aud`, `exp`, `nonce`), Discord through `/users/@me` (verified email only). The app gets the result in two halves: `start` hands the app a random `key` (kept in memory / sessionStorage), the redirect back to the app only carries a 60 s one-time `code`, and `redeem` needs both. The callback only stores the verified identity; linking, finding or creating the account happens in `redeem`, after the key check. A hijacked redirect alone gets nothing, and a victim's browser walked through an attacker's link-mode URL links nothing (the attacker has the key but the code went to the victim's browser).
- **Account linking**: a provider identity attaches to an existing account only (a) from Profile > Sécurité while signed in, or (b) automatically when both the provider *and* the account have a verified email. Never on an unverified email.
- **Legacy name grab**: `POST /api/join` without a token used to hand over any member who had the same first name. It now refuses (`409 account_protected`) when that account is secured: you must sign in. A legacy account still goes to anyone who types its first name (old behaviour, old apps need it), but that token is marked `claimed`: it plays normally, but `POST /auth/credentials`, `/auth/password`, `/auth/2fa/setup` and `/enable`, `/auth/sessions/revoke-others`, `DELETE /auth/sessions/{id}` of another session, `DELETE /auth/oauth/{p}` and `oauth/{p}/start` with `link: true` (checked again on `redeem`) answer `403 claimed_session`, unless it is the account's only session (nothing is then taken from anyone). So whoever grabs a name can't secure the account for themselves nor sign its owner out; the owner's first device can sign the grabber out. Tokens from `/api/join` that created a new member, signed-in joins, password, OAuth and reset sessions are never claimed. Tokens issued before this column existed count as not claimed.
- **Email enumeration**: the email step says whether an account exists (needed for a one-field "continue with email" screen). It is rate-limited (20 / 10 min per IP). "Forgot password" never says whether the email exists.
- **Email**: a small `Mailer` trait: Mailgun's HTTP API in production (reqwest, form post, 10 s timeout), a log-only mailer when Mailgun is not configured (body at INFO, API reports `email_sent: false`), a recording fake in tests. Every email has a text and an HTML part, French copy, the code in big type and nothing else.
- **TOTP secret at rest**: encrypted with a key derived (HKDF-SHA256) from `AUTH_SECRET`. Without `AUTH_SECRET`, the server generates one into `DATA_DIR/auth.key` (0600) on first boot and logs a warning; the database alone is then still not enough, but setting `AUTH_SECRET` explicitly and keeping it out of the data backups is better. Losing it disables everyone's 2FA secrets and recovery codes (they would have to be reset in SQL).

### Rate limits

| What | Per IP | Per account |
| --- | --- | --- |
| `auth/email` (does it exist) | 20 / 10 min | |
| `auth/signup` | 5 / hour | |
| `auth/login` | 10 / 10 min | 5 failures / 15 min (then `429`, refills one every 3 min) |
| `auth/login/2fa`, 2FA disable | 10 / 10 min | 5 failures / 15 min, plus 5 per challenge |
| `auth/forgot` | 5 / hour | 3 / hour |
| `auth/reset` | 10 / 10 min | 10 / hour per email, 5 per code |
| `auth/verify`, `oauth/redeem` | 10 / 10 min | 5 per code |
| `auth/verify/send` | | 5 / hour |
| `auth/delete/send` | 5 / hour | 3 / hour per email |
| `auth/delete/confirm` | 10 / 10 min | 10 / hour per email, 5 per code |
| `DELETE auth/account` | | password: the sign-in failure budget; 2FA code: the 2FA one |
| `reports` | | 20 / hour per account (and 30 report emails / hour server-wide) |
| `oauth/*/start` | 20 / 10 min | |
| legacy `/join` (without a token), `/start` | 10 / hour | |

Per-account keys: the lowercased email for login / forgot / reset (so unknown emails are limited the same way), the user id for 2FA. "Failures" budgets (sign-in, 2FA, password change) take their token before the password or code is checked and give it back on success, so parallel attempts can't all slip through. Per-IP limits only apply when the IP is known (always in production; in-process tests have no socket). With `TRUST_PROXY` unset behind a proxy every client shares the proxy's IP: set it.

## Endpoints

Everything under `/api/auth`. Bodies are JSON. Errors: `{"error": code}`, status `400` unless noted (`409` for `email_taken`, `already_secured`, `already_verified`, `totp_enabled`, `identity_taken`, `already_linked`, `last_method`, `account_protected`; `404` for `unknown_provider` / `not_found`; `403 no_room`, `403 claimed_session`; `410 sign_in_required`; `429 rate_limited` with a `Retry-After` header too; `503 email_not_configured`). **`401 {"error":"unauthorized"}` only ever means a missing, invalid or revoked token** (and `link: true` without one): `bad_credentials`, `bad_code` etc. are `400`, so the app can sign out on 401 safely. Session responses: `{"token", "user_id", "new": bool}`; 2FA needed instead: `{"mfa": "totp", "challenge"}`. Clients send `X-Device: <label>` (e.g. `iPhone · iOS 18`, `Web · Safari`) when they get a session, **percent-encoded** (`iPhone%20%C2%B7%20iOS%2018`: `·` is not a valid header byte on Android); the server decodes it, drops control characters and keeps 80 characters. Dates (`created_at`, `last_used_at`) are RFC 3339 UTC, `2026-11-12T10:04:05.123Z`.

| Method + path | Auth | Body | Returns / errors |
| --- | --- | --- | --- |
| `POST /auth/email` | | `email` | `{account: bool}` · `bad_email` |
| `POST /auth/signup` | | `email, password, name` | session + `email_sent` · `bad_email`, `bad_name`, `weak_password`, `common_password`, `email_taken` |
| `POST /auth/login` | | `email, password` | session or challenge · `bad_credentials` (also for a malformed email) |
| `POST /auth/login/2fa` | | `challenge, code` (TOTP or recovery code) | session · `bad_code`, `expired` |
| `POST /auth/forgot` | | `email` | `{sent: true}` always · `503 email_not_configured` |
| `POST /auth/reset` | | `email, code, password` | session or challenge (2FA still applies, unless the email was unverified: then providers and 2FA are removed, see above); all sessions revoked · `bad_code`, `expired`, password errors |
| `GET /auth/account` | yes | | `{user_id, name, email, email_verified, has_password, totp, recovery_codes_left, providers: [..], secured}` |
| `POST /auth/credentials` | yes | `email, password` | `{ok, email_sent}`: legacy account adds email + password, a verification code is emailed · `already_secured` (has an email already), `bad_email`, `email_taken`, password errors |
| `POST /auth/verify/send` | yes | | `{email_sent}` · `no_email`, `already_verified` |
| `POST /auth/verify` | yes | `code` | `{email_verified: true}` · `bad_code`, `expired` |
| `POST /auth/password` | yes | `current` (if one exists), `password`, `revoke_others` | `{ok, revoked}` · `bad_credentials`, password errors, `no_email` |
| `POST /auth/2fa/setup` | yes | | `{secret, otpauth_url}` (base32; label = email, issuer `Qui de nous ?`) · `totp_enabled`, `not_secured` (2FA needs an email + password or a provider first) |
| `POST /auth/2fa/enable` | yes | `code`, `revoke_others` | `{recovery_codes: [10], revoked}` · `bad_code`, `no_setup`, `totp_enabled` |
| `POST /auth/2fa/disable` | yes | `code` (TOTP or recovery, same field) | `{ok}` · `bad_code`, `totp_off` |
| `POST /auth/2fa/recovery` | yes | `code` (TOTP) | `{recovery_codes: [10]}` (old ones dropped) · `bad_code`, `totp_off` |
| `GET /auth/sessions` | yes | | `[{id, device, created_at, last_used_at, current}]` |
| `DELETE /auth/sessions/{id}` | yes | | `{ok}` · `404 not_found` |
| `POST /auth/sessions/revoke-others` | yes | | `{revoked}` |
| `POST /auth/logout` | yes | | `{ok}` (deletes this token) |
| `GET /auth/providers` | | | `["google", "discord"]` (the configured ones) |
| `POST /auth/oauth/{provider}/start` | optional (`link: true` needs it) | `app_redirect, link?` | `{url, key}`: open `url` in the browser, keep `key` for `redeem` · `unknown_provider`, `bad_redirect` |
| `GET /auth/oauth/{provider}/callback` | | provider redirect | 302 to `app_redirect?code=…` or `?error=…` (see below); an unknown or already used `state` gets a plain-text `400` page (no redirect target is known) |
| `POST /auth/oauth/redeem` | | `code, key` | session or challenge, or `{linked: provider}` · `bad_code` (wrong / used / expired code or wrong key), `409`: `identity_taken`, `already_linked`, `email_taken`; link mode: `403 claimed_session`, `401` if the session that started it was signed out meanwhile |
| `DELETE /auth/oauth/{provider}` | yes | | `{ok}` · `last_method` (would leave no way to sign in), `404 not_found` (not linked) |

`/api/features` gains `auth`, `email` (Mailgun configured), `oauth_google`, `oauth_discord` (each only when configured).

`GET /api/me` (and every room endpoint) for a signed-in account with no room yet answers `403 no_room` instead of `401`, so the app sends that account to "join or create a room" instead of signing it out. `POST /api/rooms` and `/api/rooms/join` work without a room (`{id, …}`). This applies to **secured** accounts only: a legacy (token-only) account left with no room still gets `401` as before, since old app builds sign out on it after a removal and that account has no other way back in.

**OAuth redirect errors** (`app_redirect?error=…`): `cancelled` (the user said no at the provider: show nothing), `provider_error` (provider error, failed code exchange, bad ID token or profile), `expired` (more than 10 min between start and callback), `server_error`. Account-level refusals (`identity_taken`, `already_linked`, `email_taken`) come back from `redeem` instead, so the app shows them where it shows other errors. A Discord account without a verified email can still sign in (new account without email) but never links by email.

## Flows

**Sign up / sign in with email** — Welcome → "Continuer avec un e-mail" → email (`/auth/email`) → password screen: sign in if the account exists, else sign up (first name + password) → verify code screen (skippable, "Plus tard"; skipped automatically when the server has no email) → no room yet: join with a code (or the invite link's) or create one.

**2FA** — Login returns a challenge → 6-digit code screen ("Utiliser un code de secours" switches the field) → session.

**Forgot password** — Password screen → "Mot de passe oublié ?" → `/auth/forgot` → code + new password → `/auth/reset` → signed in (or 2FA step). All other devices are signed out.

**OAuth** (Google and Discord, same for web and phone, no new native module; the provider code is a small generic table so Apple can be added later):

1. App calls `POST /auth/oauth/{p}/start {app_redirect}` and keeps the returned `key`. `app_redirect` is `quidenous://oauth` on the phone, `<origin>/oauth` on the web; anything else must start with `PUBLIC_URL` or be in `OAUTH_REDIRECTS`.
2. App opens `url` with `expo-web-browser` `openAuthSessionAsync` (web: same tab).
3. Provider → `PUBLIC_URL/api/auth/oauth/{p}/callback` (one https redirect URI to register per provider). The server checks `state`, exchanges the code with its own PKCE verifier + client secret, verifies the ID token (Google) or reads `/users/@me` (Discord, verified email only).
4. Server stores the verified identity (provider, subject, email, whether it is verified, name, link-mode account) as a 60 s one-time result and redirects to `app_redirect?code=…`. Nothing is linked or created yet.
5. App `POST /auth/oauth/redeem {code, key}`. Once the key matches, the server finds the account (identity → link-mode user → verified email) or creates one (name and verified email from the provider) → session (or 2FA challenge, or `linked`).


**Security screen** (Profile > Sécurité): email (+ verify), password (set/change, sign out others), 2FA (QR + secret, code, then 10 recovery codes to save), linked accounts (link/unlink), sessions (list, revoke, revoke all others), app lock (phone only).

**App lock** (phone only, `expo-local-authentication`, needs a new native build): off by default; when on, coming back to the app after 5 min (or a cold start) asks Face ID / fingerprint / device passcode before showing anything. Hidden on builds without the native module and on the web.

## Migration of existing users

- Nothing changes for current tokens: every legacy token keeps working, no sign-out. Old app versions (0.14 and earlier) keep using `/api/join` and `/api/start`.
- New app: a legacy account (no email, no provider) gets a dismissible "Sécurise ton compte" card on the home screen and an entry in the profile, leading to email + password or "link Google / Discord".
- `/api/join` by first name can no longer take over a secured account. Legacy accounts stay claimable by name until they secure themselves (that's the old behaviour; the card nudges them).
- `AUTH_REQUIRED=1` (later, once every phone runs the new build) turns off the anonymous `/api/join` and `/api/start` (`410 sign_in_required`).
- New users only see the new screens: sign in or sign up first, then join with a code or create a room (`/api/rooms/join`, `/api/rooms`).

## Configuration

| Variable | |
| --- | --- |
| `AUTH_SECRET` | 32+ random bytes (e.g. `openssl rand -hex 32`). Encrypts TOTP secrets, keys code hashes. Generated into `DATA_DIR/auth.key` if absent. |
| `PUBLIC_URL` | `https://qdn.crabznetwork.com` (no trailing slash), needed for OAuth (callback URL) and links in emails. New: the server had no such setting (share links use the `Host` header) |
| `TRUST_PROXY` | `1` behind the reverse proxy (client IP from `X-Forwarded-For`) |
| `MAILGUN_API_KEY`, `MAILGUN_DOMAIN`, `MAILGUN_FROM`, `MAILGUN_REGION` | email through Mailgun's HTTP API (`POST https://api.mailgun.net/v3/<domain>/messages`, basic auth `api:<key>`; `MAILGUN_REGION=eu` uses `api.eu.mailgun.net`, default `us`). `MAILGUN_FROM` like `Qui de nous ? <no-reply@mg.example.com>` (defaults to `Qui de nous ? <no-reply@<domain>>`). Without key + domain, emails are logged at INFO and the API says `email_sent: false` / `503 email_not_configured`: nothing pretends to have sent |
| `GOOGLE_CLIENT_ID`, `GOOGLE_CLIENT_SECRET` | Google "Web application" OAuth client, redirect URI `PUBLIC_URL/api/auth/oauth/google/callback` |
| `DISCORD_CLIENT_ID`, `DISCORD_CLIENT_SECRET` | Discord application > OAuth2, redirect `PUBLIC_URL/api/auth/oauth/discord/callback` |
| `OAUTH_REDIRECTS` | extra allowed app redirects, comma-separated prefixes (e.g. `exp://` for Expo Go in dev) |
| `AUTH_REQUIRED` | `1` = no more anonymous join/start |
| `REPORT_EMAIL` | where reports are emailed (default `contact@coflop.fr`) |

A provider is on only when all its variables (and `PUBLIC_URL`) are set.

## What Tugdual has to do

1. `AUTH_SECRET`, `PUBLIC_URL`, `TRUST_PROXY=1` in `deploy/.env` (Ravenne's homelab).
2. Mailgun → `MAILGUN_API_KEY` (Sending API key), `MAILGUN_DOMAIN`, `MAILGUN_FROM`, `MAILGUN_REGION` (`eu` if the account was created in the EU region). **Free plan caveat**: on the sandbox domain (`sandboxXXXX.mailgun.org`) Mailgun only delivers to *authorized recipients* (up to 5, each must accept an invite), so real users get nothing until a custom domain (e.g. `mg.crabznetwork.com`) is added and verified (SPF + DKIM TXT records, MX optional). Do that before opening to the public.
3. Google: Cloud console > APIs & Services > Credentials > OAuth client "Web application", redirect URI above; OAuth consent screen (app name, support email; publish it, scopes `openid email profile` need no review).
4. Discord: developer portal > New application > OAuth2: copy client id/secret, add the redirect.
5. A new native build (EAS) for the app lock (`expo-local-authentication`); everything else ships with the web app and an OTA-compatible JS update.
6. Later: `AUTH_REQUIRED=1` once the old builds are gone.

## Deleting an account, reports, blocks

Migration `0016_play.sql` (store readiness): `users.deleted_at`, `auth_codes.kind` accepts `delete_account` (table rebuilt, same indexes), new tables `reports` and `blocks`.

**Deleting an account** — from the app (Profile > Sécurité > Supprimer mon compte) or from the public page `/delete-account` (server-rendered, French, inline form: email → 6-digit code → confirm; accounts without an email are told to use the app or write to contact@coflop.fr). What happens:

1. Every active seat is given up like `/rooms/{id}/leave` (`rooms::leave_seat`): an owner hands over to the oldest other active member, alone = the room is deleted.
2. One transaction: their chat messages and the inbox entries they received or caused are deleted; every seat they ever had is anonymised (`Compte supprimé`, no photo, `user_id` NULL, `removed_at` set); sessions, recovery codes, providers, auth codes, push subscriptions, friends and blocks (both ways) are deleted; the `users` row stays (votes, packs and reports point at it) but is scrubbed: `Compte supprimé`, no email / password / 2FA / photo, an untypeable friend code, `deleted_at` set.
3. Photo files are removed from `DATA_DIR/photos`.

Votes, rankings, rounds, questions they added and packs they published stay, so the others' history and stats still add up (the seat shows as `Compte supprimé`). Reports they filed are kept. The email is free again afterwards.

**Reports** (store policy on user content) — a message, a member or a question, from inside the room it belongs to. Stored once per (reporter, kind, target) with a snapshot of the content, and emailed (text + HTML) to `REPORT_EMAIL` (default `contact@coflop.fr`).

**Blocks** — I stop seeing someone: their chat messages (`GET /rounds/{id}/messages`), inbox entries they caused (and the unread counts), pushes for their messages and nudges, and friend requests both ways (an unknown code, same answer as a wrong one; adding to a room says `not_friends`). Blocking removes a friendship by code; they leave my friends list. Shared rooms, votes and results don't change. Messages (REST and the `message_posted` live event) carry the author's `user_id` (null once deleted) so the app can filter what arrives live.

| Method + path | Auth | Body | Returns / errors |
| --- | --- | --- | --- |
| `DELETE /auth/account` | yes | `password?`, `code?` (TOTP or recovery) | `{ok}`, every session gone (next call `401`). With a password: that password, or a 2FA code if 2FA is on; 2FA without a password: a code; legacy / provider-only: nothing · `reauth_required`, `bad_credentials`, `bad_code`, `429`, `403 claimed_session` |
| `POST /auth/delete/send` | | `email` | `{sent: true}` always (nothing sent for an unknown email) · `bad_email`, `503 email_not_configured` |
| `POST /auth/delete/confirm` | | `email, code` | `{deleted: true}` · `bad_code`, `expired` (30 min, 5 tries, single use) |
| `POST /reports` | yes | `kind` (`message`/`member`/`question`), `id`, `reason` (`offensive`/`harassment`/`sexual`/`spam`/`other`), `details?` (≤ 500) | `{ok}` (also for a repeat) · `bad_report` (unknown kind / reason, details too long), `bad_target` (myself), `404 not_found` (unknown, or not in a room of mine), `429` |
| `GET /blocks` | yes | | `[{user_id, name, color, photo, photo_url}]` (`photo` = `photo_url` = `/photos/…` or null) |
| `PUT /blocks/{user_id}` | yes | | `{ok}` (idempotent) · `bad_target` (myself), `404 not_found` (unknown or deleted) |
| `DELETE /blocks/{user_id}` | yes | | `{ok}` |

`/api/features` gains `delete_account`, `reports`, `blocks`.

## Not in this round

Changing the email of an account, passkeys (WebAuthn), Sign in with Apple (left out for now; adding it = one more provider entry + an ES256 client-secret JWT, plus `expo-apple-authentication` for the native sheet), email links (codes only).
