-- The settings of a printer move into one column of JSON text, so that a new setting needs no change
-- to the database. What a printer had keeps its value, and a setting that is not there has its default.

ALTER TABLE printers ADD COLUMN settings TEXT NOT NULL DEFAULT '{}';

UPDATE printers SET settings = json_object(
    'printer_buffer', printer_buffer,
    'network_buffer', network_buffer,
    'paper_roll', paper_roll,
    'print_speed', print_speed,
    'max_connections', max_connections,
    'idle_timeout', idle_timeout
);

ALTER TABLE printers DROP COLUMN printer_buffer;
ALTER TABLE printers DROP COLUMN network_buffer;
ALTER TABLE printers DROP COLUMN paper_roll;
ALTER TABLE printers DROP COLUMN print_speed;
ALTER TABLE printers DROP COLUMN max_connections;
ALTER TABLE printers DROP COLUMN idle_timeout;
