# Plan for Stubprint

## Why
Printing from an app can't be tested without a printer. Existing emulators either draw nothing from
network input, have no API for agents, or support only one command language. This one is a library
plus a small server, so tests, people and agents can all see exactly what was printed.

## Layout
A Cargo workspace, one repo. Folders have no prefix. The packages are named `stubprint-core`,
`stubprint-render` and `stubprint-server`, and the program is called `stubprint`.

```text
crates/
  core/      package `stubprint-core`: decoders, document model, printer state (a library, no networking)
  render/    package `stubprint-render`: a receipt as text and as a picture, and the fonts
  server/    TCP listener, REST API, live events (headless binary), see docs/SERVER.md
  app/       Tauri shell around the same server and page (added last)
web/         the web page, a static single page app that only talks HTTP and events
fixtures/    specs/ recipes (committed), fixtures.lock.json, generate/ (a Node script),
             out/ the generated streams, expected results and previews (ignored)
docs/
```

Crates are local only (`publish = false`). Other projects can use the core by path or git.

## Decisions
- Rust backend. Decoding and serving in one small binary, no runtime needed.
- The page is a static app, so the same files work in a browser and in the Tauri window. It must
  not use Tauri only calls. It talks to the server over HTTP and server sent events.
- The page is React with Vite and TypeScript, for the size of its ecosystem and because the
  author already works in it. Its types and its API client are made from the server's OpenAPI
  document, so the two cannot drift apart.
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
   ports, REST, live events, the trace of every command, the connection log, `decode`.
4. Open source readiness: third party notices and a licence check, CI, contribution and security
   notes.
5. The essentials still missing from the server: adding and removing printers while it runs, the
   page's files served from the program, CORS for development, the Arabic fixture sent the way real
   programs send it, and the triage of the mutation tests.
6. The web page (`web/`), starting with the paper, the command list and the printer's status.
7. Added one by one, each used through the page: the realism of a real printer (offline hold, print
   speed, receive buffer, connection limits, idle timeouts), MCP, the decoder gaps (page mode,
   second colour, rotated characters, a few Star commands), more printer models.
8. Packaging: the Docker image, the Tauri desktop app, the first release (`0.1.0`), and the
   documentation at stubprint.dev.
