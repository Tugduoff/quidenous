# qui de nous ?

Private voting game for our group: a question appears, everyone votes for someone in the group,
the next question unlocks as soon as everyone has voted. Results live in the history.

| Folder | What |
| --- | --- |
| `server/` | Rust API (Axum + SQLite + WebSocket) |
| `app/` | Expo React Native app (Android, iPhone, web) |
| `deploy/` | Docker Compose for the homelab |
| `mockups/` | Screen mockups (light and dark) |
| `PLAN.md` | Product and technical plan |

## Server

```bash
cd server
JOIN_CODE=BANDE-42 cargo run        # http://localhost:8080/api/health
cargo test
```

Config (env): `PORT` (8080), `DATA_DIR` (`data`), `DATABASE_URL`, `GROUP_NAME`, `JOIN_CODE`, `WEB_DIR` (serves the web build).

### API (`/api`, `Authorization: Bearer <token>`)

| Method | Path | |
| --- | --- | --- |
| POST | `/join` `{code, name}` | → `{token, member}` |
| GET / PUT | `/me` `{name}` | profile |
| PUT | `/me/photo` (multipart `photo`) | resized to 256 px |
| GET | `/group` | members |
| GET | `/round/current` | open question, who voted, my vote |
| POST | `/round/current/vote` `{target_id}` | closes the round when everyone voted |
| GET | `/rounds/{id}` | full result once closed |
| POST | `/rounds/{id}/rating` `{value: 1 \| -1}` | thumbs up / down |
| GET | `/history?before=` | closed rounds, newest first |
| POST | `/questions` `{text}` · GET `/questions/queue` | our own questions, asked before the deck |
| WS | `/live?token=` | `vote_cast`, `round_closed`, `round_opened`, `member_joined`, … |

## Deploy (homelab)

See [deploy/README.md](deploy/README.md): one Docker image (API + web app), HTTPS reverse proxy with WebSockets.
