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

Commit messages follow Conventional Commits: `feat(core): ...`, `fix(server): ...`, `docs: ...`.
