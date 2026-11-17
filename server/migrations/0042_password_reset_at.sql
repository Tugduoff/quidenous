-- When the password was last reset by e-mail (unix seconds): for 7 days after it, 2FA can't be
-- reset by e-mail too (see docs/AUTH.md, *2FA lost*), or the mailbox alone would beat 2FA.
ALTER TABLE users ADD COLUMN password_reset_at INTEGER;
