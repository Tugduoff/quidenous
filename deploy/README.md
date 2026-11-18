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
cp .env.example .env                 # set JOIN_CODE, ADMIN_USER_IDS, VAPID_SUBJECT (server env lists them all)
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
    client_max_body_size 10m;            # photo and chat image uploads (8 MB)
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

**Set `TRUST_PROXY=1`** (see below) behind any of these. Otherwise every client shares the proxy's IP and the per-IP limits (anonymous `/join` and `/start`: 10 per hour, sign-in, codes) throttle everyone at once. The server reads the client IP from the **right-most** `X-Forwarded-For` entry of the **last** header line, so the proxy must append to that header or add its own line (Caddy and Traefik append by default; nginx with the `$proxy_add_x_forwarded_for` line above) and must be the only proxy in front of the container (a CDN in front of it would make everyone share the CDN's IP).

## Config (`deploy/.env`)

`.env.example` lists the variables a deployment sets; fill them in, then add any optional ones you need. Every variable the server reads, with its default and what it does, is printed by the server itself:

```bash
docker compose run --rm quidenous server env             # name, default, description
docker compose run --rm quidenous server env --example   # the contents of .env.example
```

The table lives in `server/src/platform/env.rs`; a test keeps `.env.example` equal to `server env --example`. Settings that look unsafe for production (`AUTH_REQUIRED` off, no `ADMIN_USER_IDS`, the default `JOIN_CODE` or `VAPID_SUBJECT`, …) are warned about in the log at start-up.

Accounts, 2FA and sign-in details: [docs/AUTH.md](../docs/AUTH.md).

Every response carries `X-Content-Type-Options: nosniff`, `X-Frame-Options: DENY`, `Referrer-Policy: no-referrer` and `Content-Security-Policy: frame-ancestors 'none'; base-uri 'self'; object-src 'none'`; the web app's pages get a full policy (scripts from the server only, plus the inline ones of its `index.html`, by hash, computed at start-up), and HSTS is sent when `PUBLIC_URL` is https. A reverse proxy adding the same headers is harmless. Rate limits on codes and writes: [docs/ROOMS.md](../docs/ROOMS.md#abuse-limits).

## Add questions

Questions are never asked twice. To add a big batch to one room's queue (one question per line, `#` for comments, `[classement]` and `[choix] … | A | B` like the in-app import, duplicates skipped):

```bash
cp my-questions.txt deploy/data/
docker compose run --rm quidenous server --import /data/my-questions.txt --room 1
```

`--room` is the room id (`groups.id`).

Community packs: `server seed-packs <dir> --author-id <id> [--featured] [--dry-run]` publishes a folder of pack files (the image ships the first ones in `/packs`, see [docs/PACK_FILES.md](../docs/PACK_FILES.md)). Safe to run again: packs already there are skipped.

The shared deck every room draws from is the built-in `server/deck/*.txt` only (changed through the repo); editing those files never moves questions between rooms.

## Update (deploy a release)

Production runs the newest `v*` tag, never `main`. [`deploy/deploy.sh`](deploy.sh) is the reference poller; run it every few minutes (cron or a systemd timer, as root or a user in the `docker` group that can read `deploy/data`):

```cron
*/5 * * * * cd /srv/quidenous && deploy/deploy.sh >> /var/log/quidenous-deploy.log 2>&1
```

For each new tag it:

1. **Waits for green CI** on the tag's commit: [`deploy/wait-green.sh <sha>`](wait-green.sh) exits 0 only when the latest run of each required check succeeded (`check`: server tests, `app`: app type check, `apk`: Android build), 1 when one failed (`deploy.sh` posts it once per tag, marker `deploy/.ci-red`), 2 while one is still running (silent, the next poll tries again). A red tag run blocks the deploy even if the same commit was green on `main` (a skipped run, like the weekly audit-only `server` run, doesn't count). A poller of your own only needs this one call before building. Needs `curl` and `python3`; set `GITHUB_TOKEN` (any read-only token) if you poll often (60 unauthenticated API calls an hour). Commits from before the `app` workflow existed: `REQUIRED_CHECKS="check apk"`.
2. Saves the database (`backup.sh --pre-deploy <tag>`, kept in `deploy/backups/pre-deploy/`, last 3).
3. Tags the running image `quidenous:prev`, builds the tag (`QD_VERSION`/`QD_GIT_SHA` build args, shown by `GET /api/version`) and starts it.
4. Checks `/api/health` and that `/api/version` reports the tag's commit, for up to 2 minutes. If not: **automatic rollback** to `quidenous:prev` with the pre-deploy database (a migration may have changed the schema, and an older binary refuses a newer one), exit 1. With `DEPLOY_WEBHOOK` set (Discord webhook) it posts deploys and rollbacks.

Release order, end to end: tag pushed → `check` (server tests) and `app` (type check, version matches the tag) green, and `apk` built → production runs the tag (this poller) → the `ota` job of the tag's *android* run publishes the new JS to the phones. That last job waits (up to 90 min) until `GET https://qdn.coflop.fr/api/version` reports the tag's commit or version, so new JS never meets the old server; it publishes nothing if `check` or `app` is red, or if production already runs a newer tag. A deploy that takes longer than that leaves the `ota` job red: re-run it once production runs the tag.

The deployed tag is recorded in `deploy/.deployed`; an older tag is ignored. Manual deploy: `deploy/deploy.sh v0.31.0` (still gated on CI). Manual rollback: `docker tag quidenous:prev quidenous:latest && docker compose up -d --no-build` (and, if the failed version ran migrations, restore `deploy/backups/pre-deploy/<tag>.db`, see below).

Migrations run automatically at start-up, after a copy of the database (`data/pre-migrate-<version>.db`, the last 3 kept; how to roll back: `docs/DATABASE.md`).

## Backup

Everything is in `deploy/data/`: `app.db` (SQLite), `photos/`, `chat/` (chat images, one folder per room), `updates/` (OTA bundles), `auth.key` (decrypts the 2FA secrets; only when `AUTH_SECRET` is unset) and the Firebase JSON. [`deploy/backup.sh`](backup.sh) takes a consistent snapshot (`VACUUM INTO` through `server --backup`, safe while the server runs), packs the files, copies it **off this host** and prunes old copies. Nightly, as root:

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
curl -s http://127.0.0.1:8080/api/version   # {"version":"0.31.0","sha":"<40 hex>"} of the running build
```

`GET /api/version` reports what the binary was built from, baked in at compile time by `server/build.rs`: `version` is the build's `QUIDENOUS_VERSION` (the Docker build arg `VERSION`), else `expo.version` in `app/app.json` (a tag's app.json holds its release); `sha` is the build's `QUIDENOUS_GIT_SHA` (build arg `GIT_SHA`), else `git rev-parse HEAD` of the checkout. A value the build could not find falls back to the same variable at runtime, then `dev`.

`GET /api/health` answers `200 ok` only when SQLite answers (`SELECT 1` within 2 s) and the disk holding `DATA_DIR` has at least 100 MB free; otherwise `503 {"error":"database"}` or `{"error":"disk_full"}` (and an `ERROR` log line). Docker only marks the container unhealthy: an outside monitor alerts you (see [Uptime monitoring](#uptime-monitoring)).

`docker stop` / a deploy sends SIGTERM: the server stops accepting connections and lets requests in flight finish (`init: true` in the compose file makes sure the signal reaches it); live sockets close and the apps reconnect to the new container.

At start-up the server logs a `WARN` line for each unsafe or missing production setting (`AUTH_REQUIRED` off, `ADMIN_USER_IDS` unset, placeholder `VAPID_SUBJECT`, default `JOIN_CODE`, a room still using the public code `BANDE-42`, `UPDATES_CERTIFICATE` unset while OTA publishing is on, `CLIENT_ERRORS_WEBHOOK` unset).

App crash reports (`POST /api/client-errors`) are always logged; at most 3 an hour from one IP and 30 an hour in all are also posted to `CLIENT_ERRORS_WEBHOOK`, so one client can't drown out the others.

Then send the URL to the group: Android → Chrome menu "Install app"; iPhone → Safari, Share → "Add to Home Screen".

## Production without Docker (LXC/systemd)

The production instance runs the binary under systemd in an LXC (settings in `/etc/qdn/env`), not this compose file. Build from a **git checkout of the tag**, so `build.rs` reads the tag's commit and its `app/app.json`:

```bash
git fetch --tags && git -c advice.detachedHead=false checkout --detach v0.36.0
cd server && cargo build --release --locked      # bakes version 0.36.0 and the tag's commit into the binary
```

From a source archive (no `.git`), pass both to the build: `QUIDENOUS_VERSION=0.36.0 QUIDENOUS_GIT_SHA=<the tag's commit> cargo build --release --locked`. Values baked in win over `QUIDENOUS_VERSION`/`QUIDENOUS_GIT_SHA` in the unit's environment. After the restart, check what runs:

```bash
curl -s https://<domain>/api/version   # {"version":"0.36.0","sha":"<git rev-list -n1 v0.36.0>"}
```

The release tooling waits for `/api/version` to report the tag's commit before publishing the over-the-air update, so a build that says `dev` (or an older commit) holds the OTA back. Gate the build on CI with `deploy/wait-green.sh <sha>` (see Update) and keep the previous binary and web folder to roll back.

Backups: `deploy/backup.sh` works without Docker: with `DATA_DIR=<the data folder>` it snapshots the database with the host's `python3` (`VACUUM INTO`, safe while the server runs), then copies off-host as described in [Backup](#backup). Run it nightly from a systemd timer or cron in the LXC, with an off-host destination: Proxmox backups on the same machine don't survive a disk failure.

## Uptime monitoring

Run the check from a machine **outside** the homelab (another server, a VPS, a Raspberry Pi elsewhere), so a dead LXC, host or connection is noticed too.

[`deploy/uptime-check.sh`](uptime-check.sh) GETs `/api/health` (10 s timeout) and posts to a Discord webhook after **2 failures in a row** (a 5-minute blip doesn't page anyone), then once when it answers `ok` again. Needs `bash` and `curl`. Every 5 minutes with systemd (as any user; the state file goes to `~/.local/state/quidenous-uptime` unless `UPTIME_STATE` is set):

```ini
# /etc/systemd/system/quidenous-uptime.service
[Unit]
Description=Quidenous uptime check
Wants=network-online.target
After=network-online.target

[Service]
Type=oneshot
User=monitor
Environment=UPTIME_URL=https://your.domain
# DISCORD_WEBHOOK=https://discord.com/api/webhooks/... (kept out of the unit file)
EnvironmentFile=/etc/quidenous-uptime.env
ExecStart=/opt/quidenous/uptime-check.sh

# /etc/systemd/system/quidenous-uptime.timer
[Unit]
Description=Quidenous uptime check every 5 minutes

[Timer]
OnBootSec=2min
OnUnitActiveSec=5min

[Install]
WantedBy=timers.target
```

```bash
sudo install -m 755 deploy/uptime-check.sh /opt/quidenous/uptime-check.sh
sudo systemctl daemon-reload && sudo systemctl enable --now quidenous-uptime.timer
journalctl -u quidenous-uptime   # failed checks are logged here
```

Or a hosted free monitor: [UptimeRobot](https://uptimerobot.com)'s free plan (checked 2026-11: 5-minute interval, keyword monitors, e-mail alerts, and Discord among its 5 free integrations). Add a **Keyword** monitor on `https://your.domain/api/health`, interval 5 min, alert when the keyword `ok` is **not** present (a `503` body doesn't contain it), and send alerts to e-mail or a Discord webhook.
