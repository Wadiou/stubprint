<p align="center">
  <a href="https://stubprint.dev">
    <img src="docs/banner.png" alt="Stubprint, a thermal printer simulator for development and testing" width="720">
  </a>
</p>

# Stubprint

A thermal printer simulator for development and testing, at [stubprint.dev](https://stubprint.dev).

Print to it the way you print to a real network printer. It reads the same command bytes (ESC/POS,
Star Line mode and StarPRNT), answers the same status questions, and shows exactly what would have
come out: as a picture of the paper, as text, as the list of commands with the bytes of each. There
is a web page for people, a REST API with live events for tests and agents, and a command line, so a
program can be checked without a printer, paper, or a trip to the shop.

It behaves like the printers it copies. A byte a real printer ignores is ignored, and one it would
not understand is reported with its place in the stream. Nothing is made prettier than the printer
would make it.

## Install

It is one program with its web page inside, for macOS, Linux and Windows.

```bash
# macOS and Linux
curl -fsSL https://stubprint.dev/install.sh | sh
```

```powershell
# Windows, in PowerShell
irm https://stubprint.dev/install.ps1 | iex
```

To install a version, put it in the address: `https://stubprint.dev/<version>/install.sh`, or
`install.ps1`. The installer checks what it downloads against the checksums of the release. Once it
is installed, `stubprint update` installs the latest release over it.

The files of every release are on [the releases page](https://github.com/Wadiou/stubprint/releases).

## Use it

```bash
stubprint
```

The web page is at <http://127.0.0.1:8080> and the printer listens on `127.0.0.1:9100`. In another
terminal:

```bash
stubprint print --sample cafe          # print a ready made receipt, or `print receipt.bin`
stubprint wait                         # wait for the next receipt, and show its paper as text
stubprint receipt show 1 --form picture -o receipt.png
```

Or print to it as to any network printer, and ask the API what came out:

```bash
printf 'Hello\n\x1dV\x42\x00' | nc -q1 127.0.0.1 9100
curl localhost:8080/api/receipts/1.txt
```

A test can start it, print to it, and wait for the receipt: `stubprint wait` ends with 0 when one
came, and with 3 when none did in time.

## Documentation

| | |
|---|---|
| [docs/SERVER.md](docs/SERVER.md) | the server: its printers, what a printer does, the API and live events |
| [docs/CLI.md](docs/CLI.md) | the command line, and its exit codes |
| [docs/ESCPOS.md](docs/ESCPOS.md), [docs/STAR.md](docs/STAR.md) | what is decoded of each language |
| [docs/PLAN.md](docs/PLAN.md) | the plan and what comes next |

## Development

For development, use Docker. It starts the server and the web page together:

```bash
docker compose up -d
```

The page is at <http://127.0.0.1:5173> and reloads as you edit it, the API is at
<http://127.0.0.1:8080> and the printer is on port 9100. The server is built again whenever the Rust
code changes. Docker is only for development: what is released is the program itself.

| Folder | |
|---|---|
| `crates/core` | `stubprint-core`: reads printer bytes into events, receipts and printer state |
| `crates/render` | `stubprint-render`: draws a receipt as text or as a picture |
| `crates/server` | `stubprint-server`: the `stubprint` program, with the TCP ports, the API and the commands |
| `web` | the web page of the program |
| `landing` | the page of [stubprint.dev](https://stubprint.dev) |
| `ui` | the colours, the fonts and the square corners that `web` and `landing` share |
| `samples` | ready made receipts, written with real printing libraries |
| `fixtures` | recipes for test streams, made with Python libraries, and what each must decode to |

To run the tests, generate the fixtures once (this needs Python 3 with venv):

```bash
cd fixtures/generate
python3 -m venv .venv
.venv/bin/pip install -r requirements.txt
.venv/bin/python generate.py
cd ../..
cargo test --workspace
```

## Licence

MIT, see [LICENSE](LICENSE). The licences of what it is built from are in [THIRD-PARTY.md](THIRD-PARTY.md). The captured streams in `fixtures/captured` keep the licences of the
projects they come from, see [NOTICE](fixtures/captured/NOTICE.md). The fonts keep theirs (SIL Open
Font License, in `crates/render/fonts`).

Epson, ESC/POS and Star are trademarks of their owners. This project is not affiliated with them.
It is written from the manuals they publish.
