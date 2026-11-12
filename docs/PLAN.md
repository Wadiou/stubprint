# Plan for Stubprint

## Why
Printing from an app cannot be tested without a printer. Stubprint is a library and a small server
that behave like a network receipt printer, so tests, people and agents can all see exactly what
was printed, in the command language the printer is set to.

## Layout
A Cargo workspace, one repo. Folders have no prefix. The packages are named `stubprint-core`,
`stubprint-render` and `stubprint-server`, and the program is called `stubprint`.

```text
crates/
  core/      package `stubprint-core`: decoders, document model, printer state (a library, no networking)
  render/    package `stubprint-render`: a receipt as text and as a picture, and the fonts
  server/    TCP listener, REST API, live events (headless binary), see docs/SERVER.md
web/         the web page, a static single page app that only talks HTTP and events
fixtures/    specs/ recipes (committed), fixtures.lock.json, generate/ (a Python script),
             out/ the generated streams, expected results and previews (ignored)
docs/
```

Crates are local only (`publish = false`). Other projects can use the core by path or git.

## Decisions
- Rust backend. Decoding and serving in one small binary, no runtime needed.
- The page is a static app that only talks to the server over HTTP and server sent events. At
  release its files are embedded in the program, so one file is the server and the page, and the
  page and the API it calls always come from the same build. There is no desktop app.
- It ships as plain binaries with an install script (`curl -fsSL https://stubprint.dev/install.sh |
  sh`), as a Docker image for CI and compose, and later as an npm package for Node projects. All
  three are the same program, and their versions are the project version, so a pipeline pins the
  one it was written for. This comes last, at release, not while the program is being built.
- The install script is attached to every release with its version built in, and the website
  redirects to it, so a pipeline writes the version once, in the address:
  `curl -fsSL https://stubprint.dev/v0.3.1/install.sh | sh`. `/install.sh` is the latest one. The
  script also takes `--version` or `STUBPRINT_VERSION`, checks the checksum, and fails when the
  version does not exist. Where stubprint.dev is hosted is decided later: it only needs redirect
  rules, and the GitHub address works without it.
- The program is also its own command line. `stubprint` starts the server and opens the page in
  the browser. Other commands call the API of a running server: print bytes or a sample, wait for
  the next receipt and print it, show or download a receipt, set a printer's status, decode a
  file. Their output is plain text or JSON and their exit codes tell CI whether it worked. This is
  how scripts and agents use it, so MCP is not planned and is added only if someone asks.
- The page has four screens: Live paper (the receipts, their paper and their commands), Printers,
  Samples and Settings. The connections of a printer are shown on its Printers screen, since each one
  belongs to a printer and the connection limit and idle timeout that end them are set there. Sending
  and decoding bytes have no screen: they are the command line and the API, and what was printed
  shows on the live paper. Settings shows the server's own settings to read, the API and the
  command line.
- The API has no version in its paths while the program is unstable, and breaking changes are
  listed in a changelog. `/api/v1` is added at the first stable release.
- Stubprint is for developers, who start the server and know its address and ports. It shows the
  printers and their ports as plain details, with no "connect your app" banner. It is found by
  typing its address and port, as with most receipt printers. Announcing it on the network (mDNS)
  and the search of the Epson and Star libraries can come later if someone asks for them.
- The page is React with Vite and TypeScript, for the size of its ecosystem. Its types and its API
  client are made from the server's OpenAPI document, so the two cannot drift apart.
- Stubprint behaves like the printers it copies. It adds nothing a real printer does not do. The
  extras that real printers do have, such as print speed and a receive buffer, are settings of a
  printer, come from documented behaviour, and are off by default so tests stay fast.
- Rust types are the source of truth. TypeScript types for the page are generated from them.
- One document model for every dialect. A dialect is only a decoder that fills it. Printer
  profiles set paper width, resolution and code pages.
- The dialect is chosen per listening port, like the emulation setting on a real printer.
  ESC/POS and Star Line share bytes, so guessing from the stream is unreliable.
- The model is a continuous roll first, which covers receipts and tickets. It can get a fixed
  size label page with positioned items later, if label languages (ZPL, TSPL, CPCL) are added.
- Never crash on bad input. Unknown commands are reported with their position and bytes.

## Dialects, in order
1. ESC/POS (done)
2. Star Line and StarPRNT (done)
3. Label languages (ZPL, TSPL, CPCL), only if needed

## API, for people and agents
Written in full in `docs/SERVER.md`. The description, `GET /api/openapi.json`, is made from the
code, and the types of the web page will be made from it. There can be several printers, so the
pretend state is set with `POST /api/printers/{name}`.

## Samples
The ready made receipts are made by real printing libraries, never by Stubprint's own encoding.
- A central JSON file for each receipt says what is printed (lines, styles, prices, codes,
  a picture) and its metadata: title, description, paper width, tags. It is also what the tests
  expect the decoder to find, so expected values never come from the decoder.
- Small programs, one for each library, read those files and build the bytes with the library:
  `python-escpos` in Python and `node-thermal-printer` in Node, which also writes Star. A receipt
  only has a variant for a library that can print everything in it.
- A sample is one receipt with variants. A variant records its library and its version, the
  command language it came out in, and its bytes. The name of a sample never says how it was made.
- The bytes are not committed. A lock file holds a hash of each one, so a library update that moves
  a byte is noticed. The server embeds what was generated, and a build without it has no samples.
- The server lists them (`GET /api/samples`), gives the bytes of one, and prints one to a printer.

## Tests
- Unit tests next to the code, integration tests in each crate's `tests/`.
- Every fixture is decoded whole and again with the bytes split at every position.
- A fuzz style test feeds random bytes and must never panic.
- Server tests send bytes over a real TCP connection and check the REST answers.
- Fixtures are recipes, not binaries, and they cover what the decoder reads, not a shop. The specs
  in `fixtures/specs/*.json` are grouped by feature: native text (plain, styles, fonts, rules,
  code pages), barcodes (every symbology the library writes, both command forms, and the height,
  width and text options), QR and PDF417 codes, pictures (58 and 80 mm, odd heights, several
  raster bands, the older column format), control commands (cuts, drawer pulse, alignment, feed),
  one small generic receipt printed three ways (native, picture, picture with native codes) in
  English, French and Arabic, and hand written bytes for edge cases no encoder produces.
- `python generate.py` in `fixtures/generate` builds `fixtures/out/escpos/` from them with the
  Python library `python-escpos`, which people use to print on real printers. Pictures are drawn
  with Pillow. What each stream must decode to is written in the spec, or computed from what the
  spec asks for, never read back from the library. Streams from other encoders are kept in
  `fixtures/captured/`.
- `fixtures/fixtures.lock.json` holds a hash of every generated stream, bitmap and expected
  result. The generate step fails when one differs, so a library or font update that moves a
  single dot is reviewed instead of silently changing what the tests check. Accept a reviewed
  change with `python generate.py --update-lock`.
- Nothing generated is committed. Previews for browsing go to `fixtures/out/previews/`, with
  `png/`, `hex/` and `text/` folders. Running the Rust tests needs `fixtures/out/escpos/`, so
  they need the generate step first.

## Stages
The server grows first until the essentials are there, then the web page starts, and every later
feature is added to the server and used through the page, so each one is tested by being used.

1. Done: the ESC/POS decoder with its fixtures and tests (`crates/core`).
2. Done: Star Line and StarPRNT.
3. Done: the picture and text drawing (`crates/render`) and the server (`crates/server`): TCP
   ports, REST, live events, the trace of every command, the connection log, `decode`, adding and
   removing printers while it runs, serving the page's folder, and the OpenAPI description.
4. Done: open source readiness: third party notices, a licence check, CI, contributing notes.
5. The essentials still missing from the server: the samples as described above, settings from
   environment variables, the version of the program in the description of the API, the Arabic
   fixture sent the way real programs send it, and the triage of the mutation tests.
6. The design of the web page, then the page (`web/`), starting with the paper, the command list
   and the printer's status.
7. Added one by one, each used through the page: the realism of a real printer (offline hold, the
   paper roll, the counters and the drawer, which are built, then print speed, connection limits,
   idle timeouts), the command line, the decoder gaps (page mode,
   second colour, rotated characters, a few Star commands), more printer models.
8. Release: the page's files embedded in the program, `/api/v1`, the binaries with the install script,
   the Docker image, the npm package,
   the first version (`0.1.0`), the README, and the documentation at stubprint.dev.
