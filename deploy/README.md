# Deploy · qui de nous ? (Ravenne's homelab)

One container: the Rust API + the web app (served on the same domain). SQLite + photos live in `deploy/data/`.

## Requirements

- Docker + Docker Compose
- A domain/subdomain with **HTTPS** (mandatory: PWA install + push notifications only work over HTTPS)
- A reverse proxy that forwards **WebSockets** (`/api/live`)

## First install

```bash
git clone git@github.com:Tugduoff/quidenous.git
cd quidenous/deploy
cp .env.example .env                 # set JOIN_CODE, ADMIN_USER_IDS, VAPID_SUBJECT (see Config)
mkdir -p data && sudo chown 10001:10001 data   # container runs as uid 10001
docker compose up -d --build         # first build ~5–10 min (Rust + Expo)
curl -s http://127.0.0.1:8080/api/health       # → ok
```

The container listens on `127.0.0.1:8080` only; expose it through the reverse proxy.

## Reverse proxy

Caddy:

```caddy
quidenous.example.com {
    reverse_proxy 127.0.0.1:8080
}
```

Nginx (WebSocket headers are required):

```nginx
server {
    server_name quidenous.example.com;
    client_max_body_size 10m;            # photo uploads
    location / {
        proxy_pass http://127.0.0.1:8080;
        proxy_http_version 1.1;
        proxy_set_header Upgrade $http_upgrade;
        proxy_set_header Connection "upgrade";
        proxy_set_header Host $host;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;   # client IP (TRUST_PROXY=1)
        proxy_read_timeout 1h;           # keep the live socket open
    }
}
```

Traefik: point a router at the service on port 8080; WebSockets work out of the box.

**Set `TRUST_PROXY=1`** (see below) behind any of these. Otherwise every client shares the proxy's IP and the per-IP limits (anonymous `/join` and `/start`: 10 per hour, sign-in, codes) throttle everyone at once. The server reads the client IP from the **last** `X-Forwarded-For` entry, so the proxy must append to that header (Caddy and Traefik do by default; nginx with the `$proxy_add_x_forwarded_for` line above) and must be the only proxy in front of the container.

## Config (`deploy/.env`)

| Variable | Default | |
| --- | --- | --- |
| `JOIN_CODE` | `BANDE-42` | Code of the first room, created when the database is empty. Change it (the default is public). Read on first start only: afterwards the owner gets a new code in the app |
| `GROUP_NAME` | `La bande` | Name of that first room (first start only) |
| `RUST_LOG` | `info` | `debug` to troubleshoot |
| `VAPID_PRIVATE_KEY` | — | Web Push key (iPhone/Android web app). Generate once: `docker compose run --rm quidenous server --gen-vapid` |
| `VAPID_SUBJECT` | `mailto:admin@example.com` | Contact sent to push services, e.g. `mailto:you@domain`. Set it: the default is a placeholder (warned about at start-up) |
| `FCM_SERVICE_ACCOUNT_FILE` | — | Android app notifications: path **inside the container** to the Firebase service account JSON, e.g. `/data/firebase.json` (put the file in `deploy/data/`) |
| `AUTH_SECRET` | generated into `data/auth.key` | 32+ random bytes (`openssl rand -hex 32`): encrypts 2FA secrets, keys code hashes. Set it, keep it out of backups, never change it (2FA secrets and recovery codes depend on it) |
| `PUBLIC_URL` | — | `https://your.domain` (no trailing slash). Needed for Google / Discord sign-in |
| `TRUST_PROXY` | off | `1` behind the reverse proxy (required there): client IP from the last `X-Forwarded-For` entry (rate limits). Without it every client shares the proxy's IP and the anonymous `/join`, `/start` limits throttle everyone. Never set it when the port is exposed directly |
| `MAILGUN_API_KEY`, `MAILGUN_DOMAIN` | — | Email (verification and reset codes) through Mailgun. Without them nothing is sent, "forgot password" is off, and the e-mails are only logged (the body with its code at `RUST_LOG=debug`) |
| `MAILGUN_FROM` | `Qui de nous ? <no-reply@MAILGUN_DOMAIN>` | |
| `MAILGUN_REGION` | `us` | `eu` for an EU Mailgun account |
| `MAILGUN_REPLY_TO` | — | `Reply-To` of the automatic e-mails (the sender is a no-reply address), e.g. `contact@your.domain` |
| `GOOGLE_CLIENT_ID`, `GOOGLE_CLIENT_SECRET` | — | Google sign-in (redirect URI `PUBLIC_URL/api/auth/oauth/google/callback`) |
| `DISCORD_CLIENT_ID`, `DISCORD_CLIENT_SECRET` | — | Discord sign-in (redirect `PUBLIC_URL/api/auth/oauth/discord/callback`) |
| `OAUTH_REDIRECTS` | — | Extra allowed app redirect prefixes, comma-separated (e.g. `exp://` for Expo Go) |
| `AUTH_REQUIRED` | off | `1` = no more anonymous join by first name (`/api/join`, `/api/start` answer `410 sign_in_required`). Off, anyone with a room code can get a session for a legacy (unsecured) account by typing its first name: set it once every phone runs an app with sign-in (0.15+). Warned about at start-up while off |
| `ADMIN_USER_IDS` | — | Comma-separated account ids (`users.id`, e.g. `1,7`) that moderate community packs and approve suggestions. Unset: nobody can, reported packs wait for review forever (warned about at start-up) |
| `REPORT_EMAIL` | `contact@coflop.fr` | Where in-app reports (message, member, question) are emailed, through Mailgun. At most 30 emails an hour; every report is also stored in the `reports` table |
| `UPDATES_PUBLISH_TOKEN` | — | Over-the-air app updates: the token CI uses to publish new JS bundles (same value as the GitHub secret). Without it publishing is off. Uploads are ~5 MB (`client_max_body_size 10m` is enough). See [docs/OTA.md](../docs/OTA.md) |
| `UPDATES_CERTIFICATE` | — | Path **inside the container** to the OTA code signing certificate, e.g. `/data/ota-certificate.pem` (copy `app/certs/certificate.pem` into `deploy/data/`). Recommended: publishing then refuses unsigned updates and signatures that don't match, so a leaked publish token can't stall updates |
| `CLIENT_ERRORS_WEBHOOK` | — | Discord webhook URL (channel → Integrations → Webhooks): app crashes caught by the app's error screen are posted there (30 an hour at most). Without it they are only logged (`app error`, level WARN) |

Accounts, 2FA and sign-in details: [docs/AUTH.md](../docs/AUTH.md).

## Add questions

Questions are never asked twice. To add a big batch to one room's queue (one question per line, `#` for comments, `[classement]` and `[choix] … | A | B` like the in-app import, duplicates skipped):

```bash
cp my-questions.txt deploy/data/
docker compose run --rm quidenous server --import /data/my-questions.txt --room 1
```

`--room` is the room id (`groups.id`). The shared deck every room draws from is the built-in `server/deck/*.txt` only (changed through the repo); editing those files never moves questions between rooms.

## Update (deploy a release)

Production runs the newest `v*` tag, never `main`. [`deploy/deploy.sh`](deploy.sh) is the reference poller; run it every few minutes (cron or a systemd timer, as root or a user in the `docker` group that can read `deploy/data`):

```cron
*/5 * * * * cd /srv/quidenous && deploy/deploy.sh >> /var/log/quidenous-deploy.log 2>&1
```

For each new tag it:

1. **Waits for green CI** on the tag's commit: [`deploy/wait-green.sh <sha>`](wait-green.sh) exits 0 only when the latest run of each required check succeeded (`check`: server tests, `app`: app type check, `apk`: Android build), 1 when one failed, 2 while one is still running (the next poll tries again). A red tag run blocks the deploy even if the same commit was green on `main`. A poller of your own only needs this one call before building. Needs `curl` and `python3`; set `GITHUB_TOKEN` (any read-only token) if you poll often (60 unauthenticated API calls an hour). Commits from before the `app` workflow existed: `REQUIRED_CHECKS="check apk"`.
2. Saves the database (`backup.sh --pre-deploy <tag>`, kept in `deploy/backups/pre-deploy/`, last 3).
3. Tags the running image `quidenous:prev`, builds the tag (`QD_VERSION`/`QD_GIT_SHA` build args, shown by `GET /api/version`) and starts it.
4. Checks `/api/health` and that `/api/version` reports the tag's commit, for up to 2 minutes. If not: **automatic rollback** to `quidenous:prev` with the pre-deploy database (a migration may have changed the schema, and an older binary refuses a newer one), exit 1. With `DEPLOY_WEBHOOK` set (Discord webhook) it posts deploys and rollbacks.

The deployed tag is recorded in `deploy/.deployed`; an older tag is ignored. Manual deploy: `deploy/deploy.sh v0.31.0` (still gated on CI). Manual rollback: `docker tag quidenous:prev quidenous:latest && docker compose up -d --no-build` (and, if the failed version ran migrations, restore `deploy/backups/pre-deploy/<tag>.db`, see below).

Migrations run automatically at start-up, after a copy of the database (`data/pre-migrate-<version>.db`, the last 3 kept; how to roll back: `docs/DATABASE.md`).

## Backup

Everything is in `deploy/data/`: `app.db` (SQLite), `photos/`, `updates/` (OTA bundles), `auth.key` (decrypts the 2FA secrets; only when `AUTH_SECRET` is unset) and the Firebase JSON. [`deploy/backup.sh`](backup.sh) takes a consistent snapshot (`VACUUM INTO` through `server --backup`, safe while the server runs), packs the files, copies it **off this host** and prunes old copies. Nightly, as root:

```cron
30 3 * * * cd /srv/quidenous && deploy/backup.sh >> /var/log/quidenous-backup.log 2>&1
```

Settings in `deploy/backup.env` (sourced) or the environment:

| Variable | Default | |
| --- | --- | --- |
| `BACKUP_RCLONE_DEST` | — | rclone destination, e.g. `b2:quidenous-backups` (S3, B2, SFTP, Google Drive…; a `crypt` remote encrypts at rest) |
| `BACKUP_RSYNC_DEST` | — | or rsync over ssh, e.g. `backup@nas:/srv/backups/quidenous` (key-based ssh) |
| `BACKUP_AGE_RECIPIENT` | — | an [age](https://age-encryption.org) public key: snapshots are encrypted before leaving the host (keep the private key offline, not on this machine) |
| `BACKUP_KEEP_DAYS` | `30` | off-host retention |
| `BACKUP_KEEP_LOCAL` | `3` | snapshots also kept in `deploy/backups/` (same disk: not a backup) |
| `BACKUP_SKIP_UPDATES` | — | `1` leaves `updates/` out (~150 MB; a new tag publishes OTA again) |

Without a destination it warns and keeps the snapshot locally only. Each snapshot is `qd-<UTC date>/` with `app.db.gz`, `files.tar.gz` and `SHA256SUMS` (`.age` suffixes when encrypted). A snapshot contains `auth.key` and the database together: encrypt it (age or a crypt remote) or keep the destination private. If you set `AUTH_SECRET` instead, `auth.key` doesn't exist: keep `AUTH_SECRET` in a password manager, the backups can't decrypt 2FA without it.

### Restore drill (do it once now, then every few months)

On any machine with Docker (not the production one):

```bash
rclone copy b2:quidenous-backups/qd-20261115-033000 ./restore      # or rsync / scp the snapshot
cd restore && sha256sum -c SHA256SUMS
# encrypted: for f in *.age; do age -d -i ~/age-key.txt -o "${f%.age}" "$f"; done
mkdir data && gunzip -c app.db.gz > data/app.db && tar -xzf files.tar.gz -C data
sudo chown -R 10001:10001 data
docker run --rm -p 127.0.0.1:8099:8080 -v "$PWD/data:/data" --env-file /path/to/deploy/.env quidenous:latest
curl -s http://127.0.0.1:8099/api/health                # ok
```

Then open `http://127.0.0.1:8099`, sign in with a real account (2FA included: that proves `auth.key`/`AUTH_SECRET` matches), check rooms, history and photos. Write down the date and how long it took. For a real restore, do the same in `deploy/data/` with the container stopped, then `docker compose up -d`.

Rolling back a failed deploy by hand: `docker compose stop quidenous && deploy/backup.sh --restore-pre-deploy <tag> && docker tag quidenous:prev quidenous:latest && docker compose up -d --no-build` (the failed database is kept as `data/app.db.failed-<tag>`).

## Check / logs

```bash
docker compose ps        # healthy?
docker compose logs -f
curl -s http://127.0.0.1:8080/api/version   # {"version":"0.31.0","sha":"…"} of the running build
```

At start-up the server logs a `WARN` line for each unsafe or missing production setting (`AUTH_REQUIRED` off, `ADMIN_USER_IDS` unset, placeholder `VAPID_SUBJECT`, default `JOIN_CODE`, a room still using the public code `BANDE-42`).

Then send the URL to the group: Android → Chrome menu "Install app"; iPhone → Safari, Share → "Add to Home Screen".
