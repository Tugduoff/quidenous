# qui de nous ?

A daily question for a group of friends: everyone votes for someone in the group, and the result shows once all have voted.
Around it: rooms, chat, a party mode for the TV, community question packs, stats, streaks and a home-screen widget.

| Folder | What |
| --- | --- |
| `server/` | Rust API (Axum, SQLite, WebSocket). Also serves the web build and the public pages |
| `app/` | Expo app (Android and web) |
| `deploy/` | Docker Compose and ops scripts for the homelab |
| `store/` | Google Play listing |
| `check`, `scripts/` | the checks, the same ones CI runs |

## Run it

```bash
cd server && cargo run                # http://localhost:8080/api/health
cd app && npm ci && EXPO_PUBLIC_API_URL=http://localhost:8080 npx expo start --web
```

`cargo run -- env` lists every setting the server reads from the environment, with its default.

## Check it

```bash
./check          # format, lint, tests, dead code, web export
./check smoke    # the real server and web app driven by a browser, screenshots in /tmp/qdn-smoke
```

## Deploy

One Docker image (API and web app) behind an HTTPS reverse proxy: see [deploy/README.md](deploy/README.md).
Production deploys on a `v*` tag only.

## Contributing

[AGENTS.md](AGENTS.md) is the reference, for people and coding agents alike.

## Licence

Source visible, all rights reserved: no copying, reuse or publication without written permission. See [LICENSE](LICENSE).
