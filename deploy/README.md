# Deploy

One container: the Rust API and the web app on the same domain. SQLite, photos, chat images and OTA bundles live in `deploy/data/`.

Every script here answers `--help`, and that help is the runbook (schedule, settings, rollback, restore drill):
`deploy.sh` (deploy the newest green `v*` tag), `backup.sh` (nightly off-host backup, pre-deploy snapshots, restore),
`wait-green.sh` (is CI green on a commit), `uptime-check.sh` (outside health check, with its systemd units next to it).

## First install

Needs Docker with Compose, a domain with **HTTPS** (install and push notifications need it) and a reverse proxy that forwards **WebSockets** (`/api/live`).

```bash
git clone git@github.com:Tugduoff/quidenous.git
cd quidenous/deploy
cp .env.example .env
mkdir -p data && sudo chown 10001:10001 data
docker compose up -d --build
curl -s http://127.0.0.1:8080/api/health
```

The container runs as uid 10001 and listens on `127.0.0.1:8080` only: expose it through the reverse proxy.

## Settings

`docker compose run --rm quidenous server env` lists every setting with its default and what it does; `.env.example` holds the ones a deployment sets.
At start-up the server logs a `WARN` line for each unsafe or missing production setting. Generate the web push key once with `docker compose run --rm quidenous server --gen-vapid`.

## Reverse proxy

Caddy (`reverse_proxy 127.0.0.1:8080`) and Traefik forward WebSockets out of the box. Nginx needs the upgrade headers:

```nginx
server {
    server_name quidenous.example.com;
    client_max_body_size 10m;
    location / {
        proxy_pass http://127.0.0.1:8080;
        proxy_http_version 1.1;
        proxy_set_header Upgrade $http_upgrade;
        proxy_set_header Connection "upgrade";
        proxy_set_header Host $host;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_read_timeout 1h;
    }
}
```

Set `TRUST_PROXY=1` behind the proxy, and only there: the server then reads the client IP from the right-most `X-Forwarded-For` entry.
Without it every client shares the proxy's IP and the per-IP limits throttle everyone at once. The proxy must be the only one in front of the container.

## Releases

Production runs the newest `v*` tag, never `main`: tag pushed, then `check`, `app` and `apk` green, then `deploy.sh` builds and starts the tag
(automatic rollback if it does not come up healthy), then the `ota` job publishes the new JS once `/api/version` reports the tag's commit.
Migrations run at start-up, after a copy of the database (`data/pre-migrate-<version>.db`, the last 3 kept).

```bash
docker compose ps
docker compose logs -f
curl -s http://127.0.0.1:8080/api/version
```

## Without Docker (LXC, systemd)

Build from a **git checkout of the tag**, so the binary knows its version and commit: `git checkout --detach v0.42.0 && cd server && cargo build --release --locked`.
From a source archive, pass `QUIDENOUS_VERSION` and `QUIDENOUS_GIT_SHA` to the build. A binary that reports `dev` holds the over-the-air update back.
`backup.sh` works there too with `DATA_DIR` set.

## Questions and packs

```bash
docker compose run --rm quidenous server --import /data/my-questions.txt --room 1
docker compose run --rm quidenous server seed-packs /packs --author-id 1 --featured --dry-run
```

`--import` adds one question per line to that room's queue (duplicates skipped). `seed-packs` publishes a folder of pack files and is safe to run again.

## Is it up

`GET /api/health` answers `200 ok` only when SQLite answers and the data disk has room; `uptime-check.sh` polls it from outside and alerts a Discord webhook.
A hosted keyword monitor on `https://your.domain/api/health` that alerts when `ok` is missing does the same job.
