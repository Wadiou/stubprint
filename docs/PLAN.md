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
  server/    TCP listener, REST API, live events, embedded web page (headless binary)
  app/       Tauri shell around the same server and page (added last)
web/         the web page, a static single page app that only talks HTTP and events
fixtures/    real printer byte streams, one folder per dialect
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
- `GET /api/receipts`, `GET /api/receipts/:id` as JSON, and `.png`, `.txt`, `.bin`
- `GET /api/receipts/next?timeout=10` waits for the next receipt
- `POST /api/print` sends raw bytes over HTTP
- `GET /api/events` is the live stream
- `DELETE /api/receipts` clears them
- `POST /api/printer` sets the pretend state: online, paper out, cover open, error
- `GET /api/openapi.json` describes the API

## Tests
- Unit tests next to the code, integration tests in each crate's `tests/`.
- Every fixture is decoded whole and again with the bytes split at every position.
- A fuzz style test feeds random bytes and must never panic.
- Server tests send bytes over a real TCP connection and check the REST answers.
- Fixtures are generated once with the JavaScript library `@point-of-sale/receipt-printer-encoder`,
  which writes ESC/POS, StarLine and StarPRNT, and committed as binary files. Day to day the
  project needs no Node.

## Stages
1. `crates/core` with the ESC/POS decoder, fixtures and tests.
2. `crates/server`: TCP listener, REST, events, tests.
3. `web/` page with the paper preview.
4. StarPRNT and StarLine decoders.
5. Printer state, dropped jobs and status replies.
6. Tauri shell, WebAssembly build, Docker image.

## Open
- License.
- Frontend framework: Svelte 5 with Vite unless React is preferred.
