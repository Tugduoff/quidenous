# Quidenous

A daily question for a group of friends: everyone votes for someone in the group, the result shows once all have voted.
`server/` is the Rust API (Axum, SQLite, WebSocket) and also serves the web build; `app/` is the Expo app (Android and web).

## The gate

- Run `./check` before calling anything done. CI runs the same script. `./check server`, `./check app` and `./check repo` run one area.
- Run `./check smoke` after any UI or API change, then look at the screenshots in `/tmp/qdn-smoke/`.
- **When an agent makes a mistake, fix it with a lint rule, a type or a test, not with a new line in this file.**
  This file only holds what tooling cannot check. When a rule here becomes checkable, move it into `./check` and delete it here.
- A failing check is information: fix the cause. Never weaken a rule, skip a test or add a disable to get to green.

## Layout

```
server/src/
  api.rs            composes the domain routers, nothing else
  platform/         config, db, errors, ids, i18n, rate limits, version gate
  accounts/ rooms/ rounds/ party/ chat/ community/ insights/ realtime/ site/
                    one folder per domain: its router(), handlers, queries and types
server/tests/it/    one integration test binary: common/ for setup, one module per area
server/migrations/  forward-only SQL

app/src/
  app/              routes only (file names are URLs): thin screens
  components/       one folder per feature, ui/ for shared primitives
  lib/api/          the only code that talks to the server: types, query keys, hooks, mutations per domain
  lib/hooks/        named hooks, the only place for a raw useEffect
  lib/device/       storage, push, updates and other device I/O
  lib/utils/        pure functions and their tests
  locales/          fr is the source of keys, en mirrors it

deploy/             compose file and ops scripts (each answers --help)
check, scripts/     the checks themselves
```

A new feature is a new domain folder on the server and a new feature folder in the app: copy the nearest one.
A new endpoint is a handler in its domain, a line in that domain's `router()`, a line in `server/tests/it/routes.rs`,
and a typed function or hook in `app/src/lib/api/<domain>.ts`.

## Rules tooling cannot check

- **No code comments.** The ban is checked; what to write instead is your call: a clearer name, a smaller function, a type, a named constant, a test.
- **No unsolicited markdown.** No plans, design notes or READMEs. Tests document behaviour, `--help` documents ops.
- **Installed apps keep calling the HTTP API.** Add fields and routes; do not rename or remove them. A breaking change raises the server's minimum app version in the same PR (old installs then see the "update required" screen) and is listed under a "Breaking" heading in the PR.
- **A new server capability gets a name in `/api/features`** so the app and the release scripts can tell whether production has it yet.
- **Migrations**: add a file with the next free number; never edit or delete an existing one (sqlx checksums would stop production from starting). 0049 is skipped on purpose. Open branches may hold numbers: look at `server/migrations` on the `origin/` branches of open PRs before picking.
- **Releases**: production deploys only on a `v*` tag, never from `main`. `app.json`'s version must equal the tag and `versionCode` must go up. Do not merge, tag or release unless asked.
- **Native changes cut phones off from over-the-air updates** until they install a new APK. `./check app` fails when the native runtime changed; only then decide whether the change is worth a native release, and say so in the PR.
- **Expo 58 is newer than your training data.** Read `https://docs.expo.dev/versions/v58.0.0/` or the package's own types before using an Expo API. Add dependencies with `npx expo install`. Never edit `android/` or `ios/` by hand.
- **Text people read** goes into `app/src/locales/fr` first, then `en` in natural English, not word for word.
- **Production data is real.** Never point a script or a test at the production database or URL.

## Working here

- One test: `cd server && cargo test --test it <module>::<name>`; app unit tests live next to the code as `*.test.ts`.
- Machine settings for `./check` (cargo target dir, Playwright location) are read from `~/.config/quidenous/check.env`.
- Commits are small and say why. Open pull requests as drafts and keep the description current.
