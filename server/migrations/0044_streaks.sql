-- Streaks (docs/STREAKS.md): computed from rounds and votes, nothing stored but the evening reminder.
-- The local day (days since 1970-01-01 in the member's time zone) the "your streak ends tonight" push
-- went out for this seat, so a restart never sends it twice the same evening.
ALTER TABLE members ADD COLUMN streak_reminded INTEGER;
