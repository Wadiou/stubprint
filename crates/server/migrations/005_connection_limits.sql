-- How many programs a printer takes at once, and how long it waits for one that sends nothing.
-- 0 is no limit of its own and never.

ALTER TABLE printers ADD COLUMN max_connections INTEGER NOT NULL DEFAULT 0;
ALTER TABLE printers ADD COLUMN idle_timeout INTEGER NOT NULL DEFAULT 0;
