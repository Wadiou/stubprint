# The command line

`stubprint` with no command starts the server ([the server](SERVER.md)). With a command it asks a
server that is running, through its REST API, and says what it answered. A command holds no printer
of its own, so what it shows is what the page shows.

```bash
stubprint printer list
stubprint print receipt.bin --printer front
stubprint print --sample grocery
stubprint wait --timeout 30            # prints the next receipt as text
stubprint receipt show 3 --form picture -o receipt.png
```

## Where the server is

| Option | Meaning | Default |
|---|---|---|
| `--server <url>` | the address of the server to ask. It is plain HTTP | `$STUBPRINT_SERVER`, else `http://127.0.0.1:8080` |
| `--json` | print what the server answered as JSON, exactly as its API says it, and not as text | off |

Both go anywhere on the line. The options that start a server (`--printer`, `--http`,
`--ephemeral`, `--reset`, `--ready-file`) do not go with a command.

## Commands

| Command | What it does |
|---|---|
| `print <file>` | sends a file to a printer. `-` reads what is typed in. `--printer <name>` picks it, the first one otherwise |
| `print --sample <id>` | prints a ready made sample. `--library` and `--dialect` pick the variant, the printer's own language otherwise |
| `wait` | waits for the next receipt and prints its paper as text. `--printer`, `--after <id>`, `--timeout <seconds>` (10, up to 120). `--id` prints only its number |
| `decode <file>` | reads bytes as a printer would and lists their commands, keeping nothing. `--dialect`, `--profile` |
| `printer list` | the printers and how each one is |
| `printer show <name>` | one printer with everything it is set to |
| `printer add key=value…` | adds a printer, such as `name=front listen=127.0.0.1:9100 print_speed=150` |
| `printer set <name> key=value…` | changes its settings, such as `profile=TM-T88IV idle_timeout=30` |
| `printer status <name> key=value…` | makes it report something: `online`, `cover_open`, `drawer_open`, `feed_button` (`true`, `false`), `paper` (`adequate`, `near_end`, `end`), `fault` (`cutter`, `unrecoverable`, `recoverable`, `null`) |
| `printer remove <name>` | removes it. Its receipts stay |
| `receipt list` | the receipts, with `--printer`, `--after`, `--limit` |
| `receipt show <id>` | one receipt. `--form text` (the default), `json`, `bytes`, `picture` (`--scale 1` to `4`) or `trace`. `-o <file>` writes it. `--upto <n>` shows it after its first n commands |
| `receipt remove <id>`, `receipt clear` | forget one receipt, or all, or those of `--printer` |
| `sample list`, `sample show <id>` | the samples and the libraries that wrote them |
| `model list` | the printer models |
| `connection list`, `connection show <id>` | the programs that connected |
| `info` | the version of the server, where it listens, and whether it keeps its state |
| `events` | follows the live events, one line each. With `--json`, `{"event": …, "data": …}` |

`key=value` settings are the same as `--printer` and the API take. Bytes and a picture are binary:
they go to a file with `-o`, or to a pipe, and never to a terminal.

## Exit codes

| Code | Meaning |
|---|---|
| 0 | it worked |
| 1 | it failed: the server cannot be reached, or it refused, and the message on stderr says why |
| 2 | the command was used wrong |
| 3 | `wait` found no receipt in time |

Results go to stdout and messages to stderr, so a script can read one and show the other.
