# Stubprint

A virtual receipt printer for development and testing, at [stubprint.dev](https://stubprint.dev).

Print to it the way you print to a real network printer. It reads the same command bytes (ESC/POS,
Star Line mode and StarPRNT), answers the same status questions, and shows exactly what would have
come out: as a picture of the paper, as text, as the list of commands with the bytes of each.
People get a web page and agents get a REST API, so both can see what a program printed without a
printer, paper, or a trip to the shop.

It behaves like the printers it copies. A byte a real printer ignores is ignored, and one it would
not understand is reported with its place in the stream. Nothing is made prettier than the printer
would make it.

Status: the decoders for ESC/POS, Star Line and StarPRNT, the picture and text drawing, and the
server with its API and live events are done and tested. The web page, MCP, a Docker image and a
desktop app come next. See [docs/PLAN.md](docs/PLAN.md) for the plan, [docs/SERVER.md](docs/SERVER.md)
for the server and its API, and [docs/ESCPOS.md](docs/ESCPOS.md) and [docs/STAR.md](docs/STAR.md)
for what is decoded.

## Try it

```bash
cargo run -p stubprint-server -- --printer name=till,listen=127.0.0.1:9100,dialect=escpos
```

```bash
# print something, then look at it
printf 'Hello\n\x1dV\x42\x00' | nc -q1 127.0.0.1 9100
curl localhost:8080/api/receipts/1.txt
curl localhost:8080/api/receipts/1.png -o receipt.png
```

## Layout

| Folder | |
|---|---|
| `crates/core` | `stubprint-core`: reads printer bytes into events, receipts and printer state |
| `crates/render` | `stubprint-render`: draws a receipt as text or as a picture |
| `crates/server` | `stubprint-server`: the `stubprint` program, with the TCP ports and the API |
| `fixtures` | recipes for test streams, made with Python libraries, and what each must decode to |

## Tests

```bash
# generate the fixtures once (needs Python 3 with venv), then run the tests
cd fixtures/generate
python3 -m venv .venv
.venv/bin/pip install -r requirements.txt
.venv/bin/python generate.py
cd ../..
cargo test --workspace
```

## Licence

MIT, see [LICENSE](LICENSE). The captured streams in `fixtures/captured` keep the licences of the
projects they come from, see [NOTICE](fixtures/captured/NOTICE.md). The fonts keep theirs (SIL Open
Font License, in `crates/render/fonts`).

Epson, ESC/POS and Star are trademarks of their owners. This project is not affiliated with them.
It is written from the manuals they publish.
