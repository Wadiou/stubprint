# Plan for Stubprint

## Why

Printing from an app cannot be tested without a printer. Stubprint behaves like a network receipt
printer, so tests, people and agents can all see exactly what was printed, in the command language
the printer is set to.

## Layout

A Cargo workspace in one repo. The packages are `stubprint-core`, `stubprint-render` and
`stubprint-server`, and the program is called `stubprint`.

```text
crates/core/     decoders, document model, printer state (a library, no networking)
crates/render/   a receipt as text and as a picture, and the fonts
crates/server/   the `stubprint` program: TCP ports, API, live events and the commands
web/             the web app of the program, embedded in it at release
landing/         the page of stubprint.dev
ui/              the colours, fonts and square corners that web/ and landing/ share
samples/         ready made receipts, written with real printing libraries
fixtures/        recipes for test streams, and what each must decode to
docs/            guides, reference and plans
```

## Decisions

- Rust backend: decoding and serving in one small binary, no runtime needed.
- The web app only talks to the server over HTTP and server sent events. At release its files are
  embedded in the program, so the app and the API it calls always come from the same build. There
  is no desktop app.
- The program is also its own command line. `stubprint` starts the server, and other commands call
  the API of a running server. Their output is plain text or JSON and their exit codes tell CI
  whether it worked, so scripts and agents use it directly. MCP is added only if someone asks.
- It ships as binaries with an install script. The script is attached to every release with its
  version built in, and stubprint.dev answers `/install.sh` and `/<version>/install.sh`, so a
  pipeline writes the version once, in the address. A Docker image and an npm package come later,
  with the same version as the program.
- The API has no version in its paths while the program is unstable, and `/api/v1` is added at the
  first stable release. Breaking changes are listed in the changelog.
- Rust types are the source of truth. The TypeScript types and the API client of the web app are
  made from the OpenAPI document of the server, so the two cannot drift apart.
- Stubprint behaves like the printers it copies and adds nothing a real printer does not do. The
  extras that real printers have, such as print speed and a receive buffer, are settings of a
  printer, come from documented behaviour, and are off by default so tests stay fast.
- One document model for every dialect. A dialect is only a decoder that fills it, and printer
  profiles set paper width, resolution and code pages.
- The dialect is chosen per listening port, like the emulation setting of a real printer. ESC/POS
  and Star Line share bytes, so guessing from the stream is unreliable.
- The model is a continuous roll, which covers receipts and tickets. A fixed size label page can
  come later, if label languages (ZPL, TSPL, CPCL) are added.
- Never crash on bad input. Unknown commands are reported with their position and bytes.
- Stubprint is for developers, who start it and know its address and ports. It has no "connect your
  app" banner, and announcing it on the network (mDNS) can come if someone asks.

## Samples

The ready made receipts are made by real printing libraries, never by Stubprint's own encoding.
`samples/receipts/*.json` says what each receipt prints. Small programs read them and build the
bytes with `python-escpos` and `node-thermal-printer`, and a receipt only has a variant for a
library that can print all of it. The bytes are not committed: `samples/samples.lock.json` holds a
hash of each, so a library update that moves a byte is noticed. The server embeds what was
generated.

## Tests

Unit tests sit next to the code and integration tests in each crate's `tests/`. Every fixture is
decoded whole and again with the bytes split at every position, a random bytes test must never
panic, and the server tests print over a real TCP connection. Expected values are written from the
manuals or made by a library that real programs use, never read back from the decoder. How to run
them is in [CONTRIBUTING.md](../../CONTRIBUTING.md).

## Stages

Done:

1. The ESC/POS decoder, Star Line and StarPRNT, with their fixtures and tests.
2. The drawing of a receipt, and the server: ports, API, live events, the trace of every command,
   the connection log, the samples and the realism settings of a printer.
3. The web app, the command line and `stubprint update`.
4. The release: binaries for five targets, the install scripts, checksums, and the landing page.

To come:

- The decoder gaps listed in [ESCPOS.md](../reference/ESCPOS.md) and [STAR.md](../reference/STAR.md),
  and more printer models.
- A Docker image and an npm package.
- `/api/v1` at the first stable release.
- Label languages, only if needed.
