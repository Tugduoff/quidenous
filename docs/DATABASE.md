# Database

One SQLite file, `DATA_DIR/app.db` (code: `server/src/db.rs`, migrations in `server/migrations/`).

## Settings

- WAL journal, foreign keys on, `synchronous=NORMAL` (safe with WAL: a power cut can lose the last commits, never corrupt the file; far fewer fsyncs on the SD card), busy timeout 5 s, pool of 8.
- `PRAGMA optimize` at start-up and when a pooled connection closes, so the planner has fresh statistics.

## Write transactions

Every transaction that writes starts with `db::write_tx(&pool)` (`BEGIN IMMEDIATE`), never `pool.begin()`.

Why: in WAL mode a deferred `BEGIN` that reads and then writes fails at once with `SQLITE_BUSY_SNAPSHOT` ("database is locked", a 500) if another writer committed in between, and the busy timeout doesn't help. `BEGIN IMMEDIATE` takes the write lock first and waits (up to the busy timeout) for the other writer, so concurrent votes, kicks and reorders queue up instead of failing. Read-only work uses the pool directly (or `begin()` for a consistent snapshot). Inside a write transaction, never use the pool for another write: it would wait for your own lock.

Single statements (`sqlx::query(...).execute(&pool)`) are their own transaction and don't need it.

## After the commit

Pushes, inbox entries and live events that follow a committed write are side effects: they log their errors (`routes::notify_round`, the pacing scheduler, the party sweeper) and never turn the request into an error. A retry would only get a 409 (`already_voted`), and one failing room must not stop the scheduler for the others.

## Migrations

They run at start-up, before the server listens (`db::migrate`):

1. **Repair**: a `-- no-transaction` migration is recorded by sqlx in `_sqlx_migrations` *after* its script. A crash in between leaves the schema migrated but unrecorded, and running the script again fails (`table choices already exists`): the server would crash-loop. `db::NO_TX_PROBES` lists such migrations with a query that is true once their script has run; when it is true and the record is missing, the start-up records it exactly as sqlx would (same checksum). Applied migrations can't be edited (sqlx checks their checksums), which is why 0027 is repaired this way rather than rewritten.
2. **Backup**: when migrations are pending on an existing database, `VACUUM INTO DATA_DIR/pre-migrate-<current version>.db` first (a consistent, compact copy). The newest 3 are kept. If the backup fails, the server doesn't migrate (and doesn't start).
3. sqlx runs the pending migrations.

`0051_said.sql` (docs/SAID.md) is the second such rebuild (`questions` for the `'said'` kind, `reports` for `'answer'`), with its own probe.

Rules for new migrations:

- Prefer normal (transactional) migrations: sqlx records them in the same transaction.
- A `-- no-transaction` migration (needed to turn foreign keys off for a table rebuild) must be one `BEGIN … COMMIT` block, idempotent where it can (`CREATE … IF NOT EXISTS`, `DROP … IF EXISTS`), end with nothing after `COMMIT` but `PRAGMA foreign_keys = ON`, and get a probe in `NO_TX_PROBES`.
- Never edit a migration that has shipped.

Rolling back to an older server after a migration: the old binary refuses a database with migrations it doesn't know. Stop the server, put the `pre-migrate-<version>.db` copy back as `app.db` (remove `app.db-wal` and `app.db-shm`), then start the old image. Writes made since the upgrade are lost.

## Indexes

`0028_indexes.sql` covers the hot lookups: `rounds(group_id, question_id)` ("has this room asked it?"), `rounds(group_id, number)` (history, latest round), `members(group_id)`, `questions(group_id)`, `push_subscriptions(user_id)`, `shares(round_id)`, `challenges(question_id)`, and the unread inbox rows (`inbox(member_id) WHERE read_at IS NULL`). `0043` adds `messages(member_id)` and `inbox(actor_id)` (erasing an account and the hourly guest purge delete by them). `0052` adds `votes(voter_id)`, `nudges(from_id)` and `tripot_bets(member_id)`: badges read one account's votes, nudges and bets across its seats (docs/BADGES.md). Deck questions are unique per `(text, lang)`.

## Pruning

Hourly (`guests::spawn_purger`, first run at start-up): guest accounts 30 days after their last party (docs/PARTY.md), sessions unused for 180 days (docs/AUTH.md), and the inbox (`inbox::prune`): entries older than 90 days, read or not, and everything past a seat's latest 200 (the app shows 50).

## Not pruned (product decision)

Nothing deletes old `messages` (chat images under `DATA_DIR/chat/` go with their message, account or room: `docs/CHAT_MEDIA.md`), `shares`, `stat_shares`, `nudges`, `reports`, `party_*` rows or legacy session tokens yet: how long to keep people's content is a product decision. OTA bundles are pruned per runtime version (5 kept), but folders of old runtime versions stay. Unused leftovers kept in the schema: tables `ratings`, `member_tokens`, column `members.token_hash` (still filled with a throwaway hash on every new seat).
