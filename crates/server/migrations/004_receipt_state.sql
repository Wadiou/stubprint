-- Whether a receipt ended with a cut (`complete`) or without one (`incomplete`).

ALTER TABLE receipts ADD COLUMN state TEXT NOT NULL DEFAULT 'complete';
