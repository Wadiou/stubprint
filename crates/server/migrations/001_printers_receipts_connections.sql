-- The first version of the database: the address of the API, the printers, the receipts and the
-- connections.

CREATE TABLE settings (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
);

CREATE TABLE printers (
    position INTEGER PRIMARY KEY,
    name TEXT NOT NULL UNIQUE,
    listen TEXT NOT NULL,
    dialect TEXT NOT NULL,
    profile TEXT NOT NULL,
    printer_buffer INTEGER NOT NULL,
    network_buffer INTEGER NOT NULL,
    status TEXT NOT NULL
);

-- A receipt is saved as the bytes it was made from, and is read again from them.
CREATE TABLE receipts (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    printer TEXT NOT NULL,
    dialect TEXT NOT NULL,
    profile TEXT NOT NULL,
    at INTEGER NOT NULL,
    raw BLOB NOT NULL
);

CREATE INDEX receipts_by_printer ON receipts (printer);

CREATE TABLE connections (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    printer TEXT NOT NULL,
    data TEXT NOT NULL
);
