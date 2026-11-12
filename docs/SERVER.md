# The server

`stubprint-server` is a virtual receipt printer on the network. A program prints to one of its TCP
ports, exactly as it would print to a real printer, and the REST API shows what came out. It runs
with no window, from the command line or a container, and it is also a library (`stubprint_server`)
that another program can start.

```bash
cargo run -p stubprint-server -- --printer name=till,listen=127.0.0.1:9100,dialect=escpos
```

Contents: [Running it](#running-it) · [Printers](#printers) · [What is kept](#what-is-kept) ·
[What a printer does](#what-a-printer-does) · [While it cannot print](#while-it-cannot-print) · [Paper and counters](#paper-and-counters) ·
[How much a program can send before it waits](#how-much-a-program-can-send-before-it-waits) · [The API](#the-api)

## Running it

| Option | Meaning | Default |
|---|---|---|
| `--http <address>` | where the API and the web page listen. It is saved and used from then on, until another is given. It has no password, so keep it on this machine unless the network can be trusted | what was saved, else `127.0.0.1:8080` |
| `--printer <settings>` | a printer, as `name=front,listen=127.0.0.1:9100`. Say it again for more printers. `--help` lists every setting. See [Printers](#printers) | one ESC/POS printer on `127.0.0.1:9100` |
| `--ephemeral` | keep nothing: use no database, so the printers, the receipts and the connections are gone when the server stops | |
| `--reset` | delete everything that was kept, and start again from the printers given, or the default one | |
| `--ready-file <file>` | write the addresses there once everything listens, and remove it on stop. With port 0 it is how a program finds out which port it got | |

The web page is in the program and is served at the address of the API.

The API allows no other origin (no CORS), so a web page from somewhere else cannot read the receipts
of a printer on this machine. During development the page uses a proxy to reach it. Any path that
is not the API gives the page, so its own routes work.

## Printers

Each printer has a port, and the port decides the command language, like the emulation setting of
a real printer, since the same bytes mean different things in ESC/POS and in Star Line mode.

| Setting | Meaning | Default | Can change while it runs |
|---|---|---|---|
| `name` | what the API calls it: letters, digits, `-`, `_` | `printer` | no |
| `listen` | the address of its TCP port. Port 0 picks a free one | `127.0.0.1:9100` | no |
| `dialect` | `escpos`, `starline` or `starprnt` | `escpos` | no |
| `profile` | the model, which sets the code pages, the columns and the answers to questions | `default` | yes, `PATCH /api/printers/{name}` |
| `printer_buffer` | how many bytes it holds while it cannot print. 0 holds nothing. See [While it cannot print](#while-it-cannot-print) | `4096` | yes, `PATCH` |
| `network_buffer` | how many bytes the network keeps for a program before the printer reads them. 0 leaves it to the system. See [How much a program can send before it waits](#how-much-a-program-can-send-before-it-waits) | `0` | yes, `PATCH`, for connections that open after |
| `paper_roll` | how long its paper roll is when full, in millimetres. It runs down as receipts print. 0 never runs out. See [Paper and counters](#paper-and-counters) | `0` | yes, `PATCH`, which puts in a new roll |
| status | what it reports: `online`, `cover_open`, `paper`, `drawer_open`, `feed_button`, `fault` | online, nothing wrong | yes, `POST /api/printers/{name}` |

## What is kept

Everything is kept in one SQLite database: where the API listens, the printers with their settings
and status, the receipts and the connections. It is `stubprint.db` in the state folder of the
system, `~/.local/state/stubprint/` on Linux, and the output says where it is. The server reads it
when it starts. After that it works from memory, and writes every change to the database as it
happens, without making a printer wait. Stopping the server waits until all of it is written.

| In the database | What it is |
|---|---|
| the address of the API | what `--http` last gave |
| the printers | their name, port, command language, model, sizes, status, paper roll and counters, in the order they were added |
| the receipts | the bytes each was made from, which printer read them, as which model, and when. Its lines and its picture are made again from the bytes when the server starts |
| the connections | each one when it closed: when, the pieces its bytes came in, and the receipts it printed |

The database changes over time as numbered SQL files in `crates/server/migrations/`, such as
`002_paper_roll_and_usage.sql`. The build finds them by looking in the folder, so a new file is used
without being listed anywhere, and the build fails when the numbers have a gap. A database from an
older Stubprint is brought up to date when the server starts, and keeps what it had.

The bytes a printer holds while it cannot print are not kept. A number is never used again, even for
a receipt that was removed, so a program that remembers a receipt by its number is never given
another. Receipts and connections have no limit: clear the receipts from the page or with
`DELETE /api/receipts`, and use `--reset` to start from nothing.

**Choosing how it is kept:**

| You want | Run |
|---|---|
| everything kept, in the default place | `stubprint` |
| nothing kept, a session that is gone when the server stops | `stubprint --ephemeral` |
| to start again from nothing, with the default printer or the ones given | `stubprint --reset` |

The database is always in the default place. To keep it somewhere else, such as a volume, put that
folder there: the development container mounts one at `/root/.local/state/stubprint`. A file that
is not a database of Stubprint, or was made by a newer one, stops the server and is not changed.

The two sizes were called `hold_offline` and `receive_buffer`. A `--printer` and a request that
still use those names are read as before.

**`--printer` and the database.** `--printer` is for a printer that should exist. It is added when
the database has no printer of that name, and the database wins for one it has: change the model in
the page and the next run keeps it, even if `--printer` writes another. To change a printer that is
kept, change it in the page, or use `--reset`. The address works the other way: what `--http` gives
wins over what was saved, and is saved. With `--ephemeral` there is no database, and `--printer`
says what the printers are.

- A printer that was kept is started even when the command line does not name it.
- If the port of a printer is taken when it starts, it listens on a free one and the output says
  which.
- The development container passes `--http 0.0.0.0:8080` and `--printer name=printer,listen=0.0.0.0:9100`,
  so the first run makes that printer reachable from outside the container, and the database is in a
  volume after that.

## What a printer does

- A connection is read as the printer would read it, in any number of pieces, until it closes. What
  was printed but not cut when it closes is still a receipt.
- A cut ends a receipt. Blank lines and cuts that print nothing make no receipt.
- Questions get answers on the same connection: the status bytes of ESC/POS (`DLE EOT`, `GS r`,
  `GS I`) and of Star (`ENQ`, ASB, version). A Star printer that was asked to report by itself
  does so when the status is changed through the API.
- At most 64 connections per printer, 256 MiB per connection.

## While it cannot print

A real printer that cannot print does not throw away what it is sent. It keeps it in its receive
buffer, and when the buffer is full it stops taking more, so the program that sends has to wait.
A printer here does the same, with a buffer of `printer_buffer` bytes.

**Which statuses stop it:**

| Status | Stops printing |
|---|---|
| `online` off | yes |
| `cover_open` | yes |
| `paper` at `end` | yes |
| any `fault` | yes. A fault that can be recovered from clears with `DLE ENQ` |
| `feed_button` held | yes on Epson, no on Star |
| `paper` at `near_end` | no, it only reports it |
| `drawer_open` | no, it only reports it |

**What happens while it cannot print:**

1. What is received is held. No command is read and no receipt is made.
2. The requests for the status are the exception, as on a real printer: `DLE EOT n` and `DLE ENQ n`
   in ESC/POS, and `ENQ` and `EOT` in Star Line mode. They are answered at once and taken out of
   what is held, so a program can ask why the printer is stuck. They are found in the bytes wherever
   they are, even inside the data of another command, as the printers do.
3. When the buffer is full the server stops reading the connection, so the program waits on TCP.
   The connection shows only the bytes that were read.
4. When the printer can print again, what it held is read and printed in the order it came. The
   program does not send anything again and nothing is lost.

**Good to know:**

- A program that has gone leaves its bytes in the printer, and they print when it can, as the
  receipts of that connection. There is one buffer for the printer, so those bytes take room from
  the next program.
- A connection that is open follows `printer_buffer` as it changes. Turning it off prints what was
  held.
- `held` in the printer, and the `held` event, say how many bytes it holds now.
- A request (`POST /api/print`, a sample) cannot wait, so one that does not fit is refused with 503.
- With `printer_buffer` at 0 the printer prints whatever it receives, whatever the status. That is
  what a test that does not care about the status wants.

## Paper and counters

**The paper roll.** A printer has no limit on its paper until it is given a roll with `paper_roll`,
in millimetres, such as `80000` for 80 m. Each receipt that prints takes as much paper as its
picture is long, at the dots per inch of the model (203 when the model does not say). The roll
runs down, and the sensors say so by themselves:

| Left on the roll | The printer reports |
|---|---|
| more than a tenth | `adequate` |
| a tenth or less | `near_end`, and it keeps printing |
| nothing | `end`, and it stops printing: what it is sent is held, as in [While it cannot print](#while-it-cannot-print) |

- The receipt that uses up the last of the paper is printed whole. A real printer stops in the
  middle of it and finishes it when paper is put in.
- A new roll puts the paper back and prints what was held: `PATCH /api/printers/{name}` with
  `{"paper_roll": 80000}`, or setting `paper` back to `adequate`.
- With a roll, setting `paper` by hand moves the roll to match: `near_end` leaves a tenth, `end`
  leaves nothing. Without a roll, the paper is only what is set by hand.
- `paper_left` in the printer says how much is on the roll, and the page shows it as a bar.

**The counters.** Every printer counts what it did since it was made: `usage` in the printer has
`receipts`, `cuts`, `line_feeds` (lines fed by line feeds and by feeding lines) and `paper_used`
(millimetres). They are kept with the printer, so they are there after a restart, and the page shows
them. A program can ask an Epson printer for two of them with `GS g 2 0 n 0`: `n` of 20 is the line
feeds and 50 is the cuts. The answer is `0x5F`, the number in digits, and a NUL. A number the
printer has no counter for is not answered. `printer_changed` is sent after each receipt, so a
watcher sees the counters change.

**The cash drawer.** A kick (`ESC p`, `DLE DC4`, and the Star drive commands) opens the drawer, and
it stays open until it is shut by hand: `drawer_open` is true, `GS r 2` answers 1, and setting
`drawer_open` back to false shuts it. A printer that cannot print kicks it when it prints.

## How much a program can send before it waits

A program that sends to a printer waits when nobody reads what it sent. Its bytes wait in three
places, one after the other, and the program waits when all three are full:

```
program  ->  its own system  ->  the network of the printer  ->  the printer buffer (printer_buffer)
```

Stubprint runs on a computer whose system keeps a lot for each connection, so a receipt of 70 KB goes
in without the program ever waiting, even when the printer cannot print. A real printer keeps far
less. `network_buffer` sets how much the network keeps on the printer's side, so a program that
sends a lot has to wait for the printer, as it does for a real one.

- It is set when the port opens and again when a connection opens, so a change applies to the
  connections that open after it.
- The system rounds the number. Linux keeps about twice what is asked, with a minimum.
- The program's own system keeps its share, which Stubprint cannot change. A program that sets a
  small send buffer of its own waits sooner still.
- 0 leaves the size to the system, which grows it as data arrives.

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
| `POST /api/printers` | adds a printer while the server runs, from the same settings as `--printer` (`name`, `listen`, `dialect`, `profile`, `printer_buffer`, `network_buffer`, `paper_roll`), and opens its port at once. Without `listen` the system picks a free port, and a name that is taken gets `_2`, `_3` and so on after it, which the answer says. 409 when the address is taken |
| `DELETE /api/printers/{name}` | removes a printer: its port and its connections close, and the receipts it printed stay |
| `POST /api/printers/{name}` | changes what the printer pretends: `online`, `cover_open`, `paper` (`adequate`, `near_end`, `end`), `drawer_open`, `feed_button`, `fault` (`cutter`, `unrecoverable`, `recoverable` or `null`) |
| `PATCH /api/printers/{name}` | sets the printer to another model, `{"profile": "TM-T20III"}`, as if another printer stood on its port. Receipts printed from then on are read and drawn as that model, and each receipt keeps the one it was printed on (`profile` in the receipt). It also sets how much the printer holds while it cannot print, `{"printer_buffer": 4096}`, how much the network keeps for a program, `{"network_buffer": 8192}`, and puts in a new full roll of paper, `{"paper_roll": 80000}` |
| `GET /api/events` | live events: `receipt`, `receipt_removed`, `cleared`, `status`, `connection` (each time one opens, receives or answers bytes, prints a receipt or closes, with its state then: its bytes, how many pieces, its receipts, and in `chunk` the piece that just came in), `printer_added`, `printer_changed`, `printer_removed`, `held` (how many bytes a printer holds changed), and `lagged` when a watcher was too slow. With `?detail=actions` also `action`, one for each command as it is read, with where its bytes are and what the printer believed after it. Commands are only made while someone watches them |
| `GET /api/connections`, `GET /api/connections/{id}` | the programs that connected: when, in what pieces and at what times the bytes came, how many bytes the printer answered, and which receipts they printed |
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
