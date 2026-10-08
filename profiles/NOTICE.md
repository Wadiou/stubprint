# Printer profiles

`profiles.json` is the `profiles` part of `dist/capabilities.json` of the
[escpos-printer-db](https://github.com/receipt-print-hq/escpos-printer-db) project, which python-escpos
and escpos-php also use. It says, for each printer, which code page every `ESC t` number selects, how
wide its fonts are and which commands it understands.

The data is by Michael Billington and others, and is provided under
[Creative Commons Attribution 4.0](https://creativecommons.org/licenses/by/4.0/). The licence is in
`LICENSE.md` in this folder.

It was changed: the `profiles` part is kept, and from `encodings` only the ones that have a table of
characters, since the rest only hold names, and the keys are sorted. `manuals.json` lists the links to
manuals that the comments of each model file give. Nothing is edited by hand: `scripts/upstream.py`
writes these files, and `upstream/lock.json` says which release they are.
