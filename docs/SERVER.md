# The server

`stubprint-server` is a virtual receipt printer on the network. A program prints to one of its TCP
ports, exactly as it would print to a real printer, and the REST API shows what came out. It runs
with no window, from the command line or a container, and it is also a library (`stubprint_server`)
that another program can start.

```bash
cargo run -p stubprint-server -- --printer name=till,listen=127.0.0.1:9100,dialect=escpos
```

## Printers

Each printer has a port, and the port decides the command language, like the emulation setting of
a real printer, since the same bytes mean different things in ESC/POS and in Star Line mode.

| Setting | Meaning | Default |
|---|---|---|
| `name` | what the API calls it: letters, digits, `-`, `_` | `printer` |
| `listen` | the address of its TCP port. Port 0 picks a free one | `127.0.0.1:9100` |
| `dialect` | `escpos`, `starline` or `starprnt` | `escpos` |
| `profile` | the model, which sets the code pages, the columns and the answers to questions | `generic` |

`--printer` can be given again for more printers. `--web <folder>` serves the built web page from
that folder at the address of the API, and any path that is not the API gives the page, so its own
routes work. The API allows no other origin (no CORS), so a web page from somewhere else cannot read
the receipts of a printer on this machine. During development the page uses a proxy to reach it. `--http` is where the API listens (default
`127.0.0.1:8080`), and `--keep` how many receipts are kept (default 500). `--config file.json` reads
the same settings from a file: `{"http": "...", "keep": 100, "printers": [{"name": "...", ...}]}`.

Nothing has a password, so the ports and the API listen on this machine only until told otherwise.

## What a printer does

- A connection is read as the printer would read it, in any number of pieces, until it closes. What
  was printed but not cut when it closes is still a receipt.
- A cut ends a receipt. Blank lines and cuts that print nothing make no receipt.
- Questions get answers on the same connection: the status bytes of ESC/POS (`DLE EOT`, `GS r`,
  `GS I`) and of Star (`ENQ`, ASB, version). A Star printer that was asked to report by itself
  does so when the status is changed through the API.
- At most 64 connections per printer, 256 MiB per connection.

## The API

`GET /api` says what this is and where the description is. The description of the API is made from
the code and is served at `/api/openapi.json`, with every route, parameter and field described.

| | |
|---|---|
| `GET /api/receipts` | the receipts, oldest first. `?printer=`, `?after=<id>`, `?limit=` (the latest) |
| `GET /api/receipts/{id}` | one receipt: its lines, every event, and where each unknown command was |
| `GET /api/receipts/{id}.bin` | the bytes the printer was sent for it |
| `GET /api/receipts/{id}.txt` | the paper as text. Pictures and codes are tags such as `[qr M: hello]` |
| `GET /api/receipts/{id}.png` | a picture of the paper. `?scale=1..4` |
| `?upto=N` on any of the forms | the receipt as it was after its first N events, for stepping through a job |
| `GET /api/receipts/{id}/trace` | the commands of a receipt one by one: where its bytes start and end, what it asked for, and what the printer believed after it (code page, style, alignment, margins, line spacing, tab stops). It answers why a line came out as it did |
| `GET /api/receipts/next` | waits for the next receipt: `?timeout=10` seconds, `?printer=`, `?after=<id>`. Answers 204 when none came |
| `DELETE /api/receipts` | clears them, all or `?printer=` |
| `POST /api/print` | sends the bytes of the body to a printer (`?printer=`, the first by default) and says which receipts they made and what the printer answered |
| `GET /api/printers`, `GET /api/printers/{name}` | the printers, their status and how many programs are connected |
| `POST /api/printers` | adds a printer while the server runs, from the same settings as `--printer` (`name`, `listen`, `dialect`, `profile`), and opens its port at once. 409 when the name or the address is taken |
| `DELETE /api/printers/{name}` | removes a printer: its port and its connections close, and the receipts it printed stay |
| `POST /api/printers/{name}` | changes what the printer pretends: `online`, `cover_open`, `paper` (`adequate`, `near_end`, `end`), `drawer_open`, `feed_button`, `fault` (`cutter`, `unrecoverable`, `recoverable` or `null`) |
| `GET /api/events` | live events: `receipt`, `cleared`, `status`, `connection`, `printer_added`, `printer_removed`, and `lagged` when a watcher was too slow. With `?detail=actions` also `action`, one for each command as it is read, with where its bytes are and what the printer believed after it. Commands are only made while someone watches them |
| `GET /api/connections`, `GET /api/connections/{id}` | the programs that connected: when, in what pieces and at what times the bytes came, how many bytes the printer answered, and which receipts they printed. The latest 200 are kept |
| `POST /api/decode` | reads the body as a printer would, in `?dialect=` and `?profile=`, and says every command, the answers and how many receipts. Nothing is kept. It shows what the same bytes mean to another language or model |
| `GET /api/profiles` | the models a printer can be set to |
| `GET /api/health` | `{"ok": true}` |

A test program prints, then calls `GET /api/receipts/next` and reads what came out.

## The picture

The picture has one pixel for each dot, on paper as wide as the model prints. It draws the text
in the styles it was printed in, pictures, barcodes, QR, PDF417, Aztec and Data Matrix codes, and
the characters a program drew itself. A code that cannot be drawn, such as MaxiCode or a GS1
DataBar, or a barcode with the wrong digits, is a box that says why. Not drawn: page mode,
two-colour print, rotated characters.

## Tests

`crates/server/tests/server.rs` starts the server and talks to it over real TCP and HTTP: printing
in pieces, answers on the socket, status changes reaching an open connection, live events, many
programs at once, noise, a taken port, and stopping. `crates/render` tests its drawing dot by dot
and draws every fixture.
