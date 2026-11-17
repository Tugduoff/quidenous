-- Le Tripot (docs/TRIPOT.md): bets on a question's winner with virtual chips that are worth nothing.
-- No real money, nothing to buy, nothing to cash out. A room's owner can turn it off.
ALTER TABLE groups ADD COLUMN tripot INTEGER NOT NULL DEFAULT 1;

-- One bet per member per question, placed while it is open. `payout` is set when it settles
-- (0 = lost, the stake back on a refund), `refunded` says it came back without a result.
CREATE TABLE tripot_bets (
    id INTEGER PRIMARY KEY,
    round_id INTEGER NOT NULL REFERENCES rounds(id) ON DELETE CASCADE,
    member_id INTEGER NOT NULL REFERENCES members(id) ON DELETE CASCADE,
    target_id INTEGER NOT NULL REFERENCES members(id) ON DELETE CASCADE,
    amount INTEGER NOT NULL CHECK (amount > 0),
    -- 'YYYY-MM' (UTC) the stake was taken from: its payout or refund goes back to that month.
    month TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    payout INTEGER,
    refunded INTEGER NOT NULL DEFAULT 0,
    UNIQUE (round_id, member_id)
);

-- Append-only: every chip that moves. A balance is 100 plus this month's rows.
CREATE TABLE tripot_ledger (
    id INTEGER PRIMARY KEY,
    group_id INTEGER NOT NULL REFERENCES groups(id) ON DELETE CASCADE,
    member_id INTEGER NOT NULL REFERENCES members(id) ON DELETE CASCADE,
    month TEXT NOT NULL,
    kind TEXT NOT NULL CHECK (kind IN ('daily', 'stake', 'payout', 'refund')),
    amount INTEGER NOT NULL,
    round_id INTEGER REFERENCES rounds(id) ON DELETE CASCADE,
    -- 'YYYY-MM-DD' (UTC) of a daily bonus: one per member per day.
    day TEXT,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);
CREATE INDEX tripot_ledger_member ON tripot_ledger (member_id, month);
CREATE INDEX tripot_ledger_room ON tripot_ledger (group_id, month);
CREATE UNIQUE INDEX tripot_ledger_daily ON tripot_ledger (member_id, day) WHERE kind = 'daily';
