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
| `hold_offline` | how many bytes it holds while it cannot print, see below. 0 holds nothing | `0` |

`--printer` can be given again for more printers. `--web <folder>` serves the built web page from
that folder at the address of the API, and any path that is not the API gives the page, so its own
routes work. The API allows no other origin (no CORS), so a web page from somewhere else cannot read
the receipts of a printer on this machine. During development the page uses a proxy to reach it. `--http` is where the API listens (default
`127.0.0.1:8080`), and `--keep` how many receipts are kept (default 500). `--config file.json` reads
the same settings from a file: `{"http": "...", "keep": 100, "printers": [{"name": "...", ...}]}`.

### Keeping the printers between runs

The printers are kept between runs. The server writes them to a file in the state folder of the
system (`~/.local/state/stubprint/state.json` on Linux) when it starts and after every change: a
printer added or removed, its model, its `hold_offline` size and what it reports. A run that is
given no printers starts the ones in the file, each as it was, so what was set in the page is still
there. The receipts, the connections and what a printer holds are not kept.

- Printers that are given (`--printer`, or `printers` in a configuration) win over the file, and are
  what is kept from then on.
- `--clear-state` forgets what was kept: it starts with the printers given, or the default one.
- `--no-state` keeps nothing and leaves the file as it is, for CI and read-only folders.
- `--state file.json` (`state_file` in a configuration) keeps them in another file. A configuration
  file keeps nothing unless it names `state_file`.
- If the port of a printer is taken when it starts again, it listens on a free one and the output
  says which. A file that is not a state file stops the server and is not changed.

## What a printer does

- A connection is read as the printer would read it, in any number of pieces, until it closes. What
  was printed but not cut when it closes is still a receipt.
- A cut ends a receipt. Blank lines and cuts that print nothing make no receipt.
- Questions get answers on the same connection: the status bytes of ESC/POS (`DLE EOT`, `GS r`,
  `GS I`) and of Star (`ENQ`, ASB, version). A Star printer that was asked to report by itself
  does so when the status is changed through the API.
- At most 64 connections per printer, 256 MiB per connection.

### While it cannot print

A real printer that is off line, has its cover open, is out of paper or has a fault does not print.
What it is sent goes into its receive buffer, and when the buffer is full it stops taking more, so
the program that sends has to wait. Set `hold_offline` to the size of the buffer to get the same:
the status that stops it is `online` off, `cover_open`, `paper` at `end` or a `fault`, and on an
Epson printer a held `feed_button` too.

- While it cannot print, what is received is held, not read. No receipt is made and no command is
  decoded. Nothing about the status changes this when `hold_offline` is 0, the default, so tests
  stay as they were.
- The requests for the status are the exception, as on a real printer: `DLE EOT n` and `DLE ENQ n`
  in ESC/POS, and `ENQ` and `EOT` in Star Line mode, are answered at once and taken out of what is
  held, so a program can ask why the printer is stuck. They are found in the bytes wherever they
  are, even inside the data of another command, as the printers do. `DLE ENQ` still recovers a fault
  that can be recovered from.
- When the buffer is full the server stops reading the connection, so the program waits on TCP.
  While it waits the connection shows only the bytes that were read.
- When the printer can print again, what it held is read and printed in the order it came. A
  program that has gone leaves its bytes in the printer, and they print when it can, as the receipts
  of that connection. The buffer of the printer is one, so those bytes take room from the next
  program.
- `held` in the printer, and the `held` event, say how many bytes it holds now, with what programs that have gone left in it.
- A connection keeps the size it had when it opened. A request (`POST /api/print`, a sample) cannot
  wait, so one that does not fit is refused with 503.

## The API

`GET /api` says what this is and where the description is. The description of the API is made from
the code and is served at `/api/openapi.json`, with every route, parameter and field described.

| | |
|---|---|
| `GET /api/receipts` | the receipts, oldest first. `?printer=`, `?after=<id>`, `?limit=` (the latest) |
| `GET /api/receipts/{id}` | one receipt: its lines, every event, and where each unknown command was |
| `DELETE /api/receipts/{id}` | removes one receipt. Its number is not used again |
| `GET /api/receipts/{id}/bytes` | the bytes the printer was sent for it |
| `GET /api/receipts/{id}/text` | the paper as text. Pictures and codes are tags such as `[qr M: hello]` |
| `GET /api/receipts/{id}/picture` | a picture of the paper. `?scale=1..4` |
| `?upto=N` on any of the forms | the receipt as it was after its first N events, for stepping through a job |
| `GET /api/receipts/{id}/trace` | the commands of a receipt one by one: where its bytes start and end, what it asked for, and what the printer believed after it (code page, style, alignment, margins, line spacing, tab stops). It answers why a line came out as it did |
| `GET /api/receipts/next` | waits for the next receipt: `?timeout=10` seconds, `?printer=`, `?after=<id>`. Answers 204 when none came |
| `DELETE /api/receipts` | clears them, all or `?printer=` |
| `POST /api/print` | sends the bytes of the body to a printer (`?printer=`, the first by default) and says which receipts they made and what the printer answered. 503 when the printer cannot print and has no room to hold them |
| `GET /api/samples`, `GET /api/samples/{id}` | the ready made receipts (shop, kitchen, bakery, ticket, text styles, barcodes, a Star one), each in every library that can print it, with the library, its version, the command language and the size of each variant. Each says the printer model it was written for (`profile`). One sample also says what is printed, as data, and what a decoder must find on the paper. |
| `GET /api/samples/{id}/bytes`, `/picture`, `/text` | the bytes of one variant (`?library=`, `?dialect=`), to send to any printer, and a picture and the text of what it prints on the paper of the model it was written for, without printing it |
| `POST /api/samples/{id}/print` | prints a sample to a printer (`?printer=`, the first by default) like `POST /api/print`, with a variant in the printer's own command language unless `?dialect=` or `?library=` say otherwise |
| `GET /api/network` | the addresses of this computer on its networks. They are what a program on another device connects to, and what a printer can listen on |
| `GET /api/printers`, `GET /api/printers/{name}` | the printers, their status and how many programs are connected |
| `POST /api/printers` | adds a printer while the server runs, from the same settings as `--printer` (`name`, `listen`, `dialect`, `profile`, `hold_offline`), and opens its port at once. Without `listen` the system picks a free port, and a name that is taken gets `_2`, `_3` and so on after it, which the answer says. 409 when the address is taken |
| `DELETE /api/printers/{name}` | removes a printer: its port and its connections close, and the receipts it printed stay |
| `POST /api/printers/{name}` | changes what the printer pretends: `online`, `cover_open`, `paper` (`adequate`, `near_end`, `end`), `drawer_open`, `feed_button`, `fault` (`cutter`, `unrecoverable`, `recoverable` or `null`) |
| `PATCH /api/printers/{name}` | sets the printer to another model, `{"profile": "TM-T20III"}`, as if another printer stood on its port. Receipts printed from then on are read and drawn as that model, and each receipt keeps the one it was printed on (`profile` in the receipt). It also sets how much the printer holds while it cannot print, `{"hold_offline": 4096}` |
| `GET /api/events` | live events: `receipt`, `receipt_removed`, `cleared`, `status`, `connection` (each time one opens, receives or answers bytes, prints a receipt or closes, with its state then: its bytes, how many pieces, its receipts, and in `chunk` the piece that just came in), `printer_added`, `printer_changed`, `printer_removed`, `held` (how many bytes a printer holds changed), and `lagged` when a watcher was too slow. With `?detail=actions` also `action`, one for each command as it is read, with where its bytes are and what the printer believed after it. Commands are only made while someone watches them |
| `GET /api/connections`, `GET /api/connections/{id}` | the programs that connected: when, in what pieces and at what times the bytes came, how many bytes the printer answered, and which receipts they printed. The latest 200 are kept |
| `POST /api/decode` | reads the body as a printer would, in `?dialect=` and `?profile=`, and says every command, the answers and how many receipts. Nothing is kept. It shows what the same bytes mean to another language or model |
| `GET /api/profiles` | the models a printer can be set to, with their maker, columns, paper width in millimetres and dots, dpi, code pages, features and links to manuals |
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
