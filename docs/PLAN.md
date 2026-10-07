# Plan

## Why
Printing from an app can't be tested without a printer. Existing emulators either draw nothing from
network input, have no API for agents, or support only one command language. This one is a library
plus a small server, so tests, people and agents can all see exactly what was printed.

## Layout
A Cargo workspace, one repo. Folders have no prefix. The core library's package is named
`receipt`, because Rust already has its own crate called `core`.

```text
crates/
  core/      package `receipt`: decoders, document model, printer state (a library, no networking)
  render/    package `receipt-render`: a receipt as text and as a picture, and the fonts
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
- Rust types are the source of truth. TypeScript types for the page are generated from them.
- One document model for every dialect. A dialect is only a decoder that fills it. Printer
  profiles set paper width, resolution and code pages.
- The dialect is chosen per listening port, like the emulation setting on a real printer.
  ESC/POS and Star Line share bytes, so guessing from the stream is unreliable.
- The model is a continuous roll first, which covers receipts and tickets. It can get a fixed
  size label page with positioned items later, if label languages (ZPL, TSPL, CPCL) are added.
- Never crash on bad input. Unknown commands are reported with their position and bytes.

## Dialects, in order
1. ESC/POS
2. StarPRNT, then StarLine
3. Label languages, only if needed

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
1. `crates/core` with the ESC/POS decoder, fixtures and tests.
2. `crates/render` and `crates/server`: pictures, TCP listener, REST, events, tests. Done.
3. `web/` page with the paper preview.
4. StarPRNT and StarLine decoders.
5. Printer state, dropped jobs and status replies.
6. Tauri shell, WebAssembly build, Docker image.

## Open
- License.
- Frontend framework: Svelte 5 with Vite unless React is preferred.
