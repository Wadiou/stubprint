-- The paper roll of a printer, in millimetres, and what it printed on it.

ALTER TABLE printers ADD COLUMN paper_roll INTEGER NOT NULL DEFAULT 0;
ALTER TABLE printers ADD COLUMN paper_left REAL NOT NULL DEFAULT 0;
ALTER TABLE printers ADD COLUMN receipts INTEGER NOT NULL DEFAULT 0;
ALTER TABLE printers ADD COLUMN cuts INTEGER NOT NULL DEFAULT 0;
ALTER TABLE printers ADD COLUMN line_feeds INTEGER NOT NULL DEFAULT 0;
ALTER TABLE printers ADD COLUMN paper_used REAL NOT NULL DEFAULT 0;
