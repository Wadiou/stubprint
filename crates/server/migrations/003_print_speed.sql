-- How fast a printer prints, in millimetres of paper a second. 0 prints without taking any time.

ALTER TABLE printers ADD COLUMN print_speed INTEGER NOT NULL DEFAULT 0;
