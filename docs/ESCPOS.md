# ESC/POS coverage

What `crates/core` decodes today. Anything else is reported as an unknown event with its position
and bytes, and decoding goes on. A real printer ignores unknown commands silently, so the report is
only a hint.

| Command | Bytes | Result |
|---------|-------|--------|
| Initialize | `ESC @` | `Initialize`, and every setting returns to its default |
| Align | `ESC a n` | `Align`, applied to the text after it |
| Feed | `ESC d n` | `Feed` of n lines |
| Bold | `ESC E n` | text style |
| Underline | `ESC - n` | text style: off, thin or thick |
| Font | `ESC M n` | `Font`, A or B, and text style |
| Size | `GS ! n` | text style: width and height from 1 to 8 |
| Invert | `GS B n` | text style |
| Code page | `ESC t n` | how the following text bytes are read |
| Line spacing | `ESC 2`, `ESC 3 n` | read and ignored |
| Cancel Kanji | `FS .` | read and ignored |
| Line feed, carriage return | `LF`, `CR` | ends a line; a carriage return is ignored |
| Raster picture | `GS v 0` | `Image`, one band per command |
| Column picture | `ESC * m` | `Image`, one strip per command, modes 0, 1, 32 and 33 |
| Barcode settings | `GS h`, `GS w`, `GS H`, `GS f` | apply to the next barcode |
| Barcode | `GS k` | `Barcode`: UPC-A, UPC-E, EAN-13, EAN-8, Code 39, ITF, Codabar, Code 93, Code 128, in both command forms |
| QR code | `GS ( k`, symbol 49 | `Qr`: model 1 or 2, size, error level, data |
| PDF417 | `GS ( k`, symbol 48 | `Pdf417`: data. Its columns, rows, size and level are read and ignored |
| Cash drawer | `ESC p` | `Pulse` in milliseconds |
| Cut | `GS V m` | `Cut`, full or partial, for m of 0, 1, 48 and 49 |

## How a stream is read

- Bytes can arrive in any number of pieces. A half received command waits for the rest, and the
  result does not depend on where the stream was cut.
- Text is collected until a line feed, a style change, or any command that prints, then read in the
  current code page.
- A receipt ends at a cut, or when the stream ends with something unprinted. Blank lines and
  carriage returns after a cut print nothing, so they are not a receipt.
- An unknown command is reported once, together with the control bytes that follow it.

## Not read yet

Scaled raster pictures (`GS v 0` with a mode above 0), the cut forms with a feed (`GS V 65` and up),
`ESC !`, margins and print area (`GS L`, `GS W`), tab stops, rotation, DataMatrix and other 2D
symbols, status replies, and the Star command sets.

## Code pages

The numbers are the Epson table that receipt libraries write: 0 is CP437, 2 CP850, 16 Windows 1252,
19 CP858, 32 CP720, 37 CP864, 50 Windows 1256, and others. A number that is not known is read as
Windows 1252.
