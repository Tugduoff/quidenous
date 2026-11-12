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
cp .env.example .env                 # set JOIN_CODE (shared with the group only)
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
| `JOIN_CODE` | `BANDE-42` | Code to join the group. Change it. |
| `GROUP_NAME` | `La bande` | |
| `RUST_LOG` | `info` | `debug` to troubleshoot |
| `VAPID_PRIVATE_KEY` | — | Web Push key (iPhone/Android web app). Generate once: `docker compose run --rm quidenous server --gen-vapid` |
| `VAPID_SUBJECT` | `mailto:admin@example.com` | Contact sent to push services, e.g. `mailto:you@domain` |
| `FCM_SERVICE_ACCOUNT_FILE` | — | Android app notifications: path **inside the container** to the Firebase service account JSON, e.g. `/data/firebase.json` (put the file in `deploy/data/`) |
| `AUTH_SECRET` | generated into `data/auth.key` | 32+ random bytes (`openssl rand -hex 32`): encrypts 2FA secrets, keys code hashes. Set it, keep it out of backups, never change it (2FA secrets and recovery codes depend on it) |
| `PUBLIC_URL` | — | `https://your.domain` (no trailing slash). Needed for Google / Discord sign-in |
| `TRUST_PROXY` | off | `1` behind the reverse proxy (required there): client IP from the last `X-Forwarded-For` entry (rate limits). Without it every client shares the proxy's IP and the anonymous `/join`, `/start` limits throttle everyone. Never set it when the port is exposed directly |
| `MAILGUN_API_KEY`, `MAILGUN_DOMAIN` | — | Email (verification and reset codes) through Mailgun. Without them codes are only logged and "forgot password" is off |
| `MAILGUN_FROM` | `Qui de nous ? <no-reply@MAILGUN_DOMAIN>` | |
| `MAILGUN_REGION` | `us` | `eu` for an EU Mailgun account |
| `GOOGLE_CLIENT_ID`, `GOOGLE_CLIENT_SECRET` | — | Google sign-in (redirect URI `PUBLIC_URL/api/auth/oauth/google/callback`) |
| `DISCORD_CLIENT_ID`, `DISCORD_CLIENT_SECRET` | — | Discord sign-in (redirect `PUBLIC_URL/api/auth/oauth/discord/callback`) |
| `OAUTH_REDIRECTS` | — | Extra allowed app redirect prefixes, comma-separated (e.g. `exp://` for Expo Go) |
| `AUTH_REQUIRED` | off | `1` = no more anonymous join by first name (`/api/join`, `/api/start` answer 410). Only once every phone runs an app with sign-in |
| `REPORT_EMAIL` | `contact@coflop.fr` | Where in-app reports (message, member, question) are emailed, through Mailgun. At most 30 emails an hour; every report is also stored in the `reports` table |

Accounts, 2FA and sign-in details: [docs/AUTH.md](../docs/AUTH.md).

## Add questions

Questions are never asked twice. To add a big batch (one question per line, `#` for comments, duplicates skipped):

```bash
cp my-questions.txt deploy/data/
docker compose run --rm quidenous server --import /data/my-questions.txt
```

## Update

```bash
cd quidenous && git pull && cd deploy && docker compose up -d --build
```

Migrations run automatically at start-up.

## Backup

Everything is in `deploy/data/` (`app.db` + `photos/`). Hot backup of the database:

```bash
sqlite3 deploy/data/app.db ".backup deploy/data/backup-$(date +%F).db"
```

## Check / logs

```bash
docker compose ps        # healthy?
docker compose logs -f
```

Then send the URL to the group: Android → Chrome menu "Install app"; iPhone → Safari, Share → "Add to Home Screen".
