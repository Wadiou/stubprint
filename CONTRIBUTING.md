# Contributing to Stubprint

Stubprint should behave like the printers it copies. That is the rule everything else follows.

- **Behaviour comes from the manuals.** A command is read the way the manual of the printer's
  command language says. The manuals are published by Epson and Star, and the files that say which
  page and command a rule comes from are in `docs/`. Do not add what no real printer does, such as
  making text prettier or reading a stream more kindly than a printer would.
- **Expected values never come from the decoder.** A test says what a stream must give, and the
  expected values are written from the manual or made by a library that real programs use
  (`python-escpos`), not read back from Stubprint's own output. The recipes are in
  `fixtures/specs`, and `fixtures/generate/generate.py` builds the streams.
- **Do not copy code from projects without a licence** that allows it. Streams other people's
  programs wrote may be added to `fixtures/captured` with their licence next to them.
- **Never crash on bad input.** What cannot be read is reported with its place and its bytes, and
  every stream must decode the same whatever the way it is cut into pieces.
- **Change the structure, do not glue.** When a feature needs a different shape, change the shape.
  Do not add a helper on top.

## Working on it

```bash
cd fixtures/generate && python3 -m venv .venv && .venv/bin/pip install -r requirements.txt fonttools
.venv/bin/python generate.py && .venv/bin/python check_glyphs.py && cd ../..
cargo fmt --all && cargo clippy --workspace --all-targets && cargo test --workspace
```

`generate.py` stops when a stream differs from `fixtures/fixtures.lock.json`. Look at what changed
in `fixtures/out/previews`, and when the change is right, accept it with
`python generate.py --update-lock` in the same commit.

## The web page

`web/` is a React page that only talks to the server's API. Its client, types and TanStack Query options
are generated from the API description (Hey API), so after changing the server, rebuild it and run
`pnpm api`. The names follow the `operation_id` of each route.

```bash
cargo build -p stubprint-server
cd web && pnpm install
pnpm dev                                  # with a stubprint running on 127.0.0.1:8080
                                          # or STUBPRINT_SERVER=http://127.0.0.1:41233 pnpm dev
pnpm build && cargo build -p stubprint-server   # the program carries the build
../target/debug/stubprint                  # and serves it, or `--web dist` to serve a folder instead
pnpm typecheck && pnpm test
```

## Samples

The ready made receipts the server ships are made by real printing libraries, never by Stubprint.
`samples/receipts/*.json` says what each receipt prints and what it is called. Two small programs
print them: `samples/python/build.py` with python-escpos and `samples/node/build.mjs` with
node-thermal-printer, and each receipt lists the libraries that print it. `samples/samples.lock.json`
holds a hash of every result, so a library update that moves a byte is reviewed.

```bash
cd samples/python && python3 -m venv .venv && .venv/bin/pip install -r requirements.txt && cd ../..
(cd samples/node && npm ci)
samples/python/.venv/bin/python samples/python/build.py        # add --update-lock to accept a change
```

It writes `samples/out/`: the bytes and a picture of every variant in `python/` and `node/`, and
`samples.json`, which the server embeds when it is built. Nothing in `out/` is committed, and a
build without it has no samples, so the sample tests need this step first.

Commit messages follow Conventional Commits: `feat(core): ...`, `fix(server): ...`, `docs: ...`.

## Releases

Releases are made by semantic-release after CI passes on `main`, from the commit messages: `feat`
makes a minor version, `fix`, `perf` and `refactor` a patch, and the rest make none. A breaking
change is a `BREAKING CHANGE:` footer, which also makes a minor version while Stubprint is below
1.0.0. A `!` after the type is not read. The release commit changes `Cargo.toml`, `Cargo.lock` and
`CHANGELOG.md`, and tags `vX.Y.Z`. The first release continues from the tag `v0.0.1`.
