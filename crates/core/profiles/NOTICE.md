# Printer profiles

`profiles.json` is the `profiles` part of the printer capability database used by
[escpos-php](https://github.com/mike42/escpos-php) and python-escpos, which is itself the
[escpos-printer-db](https://github.com/receipt-print-hq/escpos-printer-db) project. It says, for
each printer, which code page every `ESC t` number selects, how wide its fonts are and which
commands it understands.

The data is MIT licensed, see `LICENSE.md` in this folder. It is copied as it is, with the keys
sorted, and without the `encodings` part, which only holds names.
