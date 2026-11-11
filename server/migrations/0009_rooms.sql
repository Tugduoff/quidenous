-- A deleted room disappears for everyone; its data stays on disk.
ALTER TABLE groups ADD COLUMN deleted_at TEXT;
