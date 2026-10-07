# Coverage plan

How `crates/core` grows from "what one encoder library writes" to "what real printers receive".

Written after reading the eight reference projects in `tmp/` and counting the commands in 25 real
captured streams. Nothing here is built yet.

## 1. What was read

| Project | Language | Licence | What it gave us |
|---------|----------|---------|-----------------|
| `escpos-php` | PHP | MIT | An encoder (every byte sequence it writes), 12 captured streams, the printer profile database |
| `python-escpos` | Python | MIT | A second encoder, the constants file, the same profile database, status handling |
| `escpos-tools` | PHP | MIT | A full command tree (`esc2html`), `GS ( L` graphics function numbers, a captured receipt with logo |
| `escpresso` | Rust | MIT | A decoder, 11 captured streams from a CUPS receipt driver, replies to status requests |
| `CrossEscPosEmulator` | C# | MIT | A decoder, status byte layouts, page mode, user defined characters, downloaded bit images |
| `escpos-virtual-printer-emulator` | Rust | MIT | A decoder, but smaller than ours. Nothing to learn from it |
| `escpos-emulator` | TypeScript | none | A decoder. Read for its list of commands only |
| `EscPosEmulator` | C# | none | A decoder. Read for its list of commands only |

Rule for the two without a licence: we read them to learn *which commands exist*. The bytes of a
command are a fact, not their work. We write every line ourselves, from the Epson command reference.
We copy no code from any project. We copy test data only from the MIT ones, with their notice.

Most of these emulators are thinner than ours. Their value is the list of commands and the captured
streams, not their code.

## 2. What real streams contain

A rough count over every captured stream (`escpos-php` outputs, `escpresso` raw files,
`escpos-tools` receipt). The counter skips picture payloads but is approximate.

| Command | Seen | We read it today |
|---------|-----:|------------------|
| `GS V 65 n`, `GS V 66 n` (cut with feed) | 38 | **no.** Reported as unknown, so the receipt does not end at the cut |
| `ESC !` (print mode: font, bold, size, underline in one byte) | 103 | **no**, style is lost |
| `GS ( L` (graphics, functions 112 and 50) | 10 | **no**, the logo is missing |
| `ESC $`, `ESC \` (absolute and relative position) | 155 | no |
| `GS L`, `GS W` (left margin, print area width) | 209 | no |
| `FS -`, `FS (`, `FS S`, `FS C` | 208 | no, and their parameter bytes print as text |
| `ESC {` (upside down), `ESC G` (double strike), `ESC e` (reverse feed) | 12 | no |
| `ESC &`, `ESC %` (user defined characters) | 9 | no |
| `GS a`, `GS r`, `DLE EOT` (status, expects a reply) | 15 | no, and nothing is sent back |
| `BS`, `EOT` (control bytes inside text) | 8 | reported as unknown |
| `GS v 0` with a scale mode | | the mode is ignored |
| `ESC E`, `ESC -`, `ESC a`, `ESC t`, `ESC M`, `ESC d`, `ESC @`, `ESC p`, `GS !`, `GS B`, `GS h/w/H/k`, `GS ( k`, `GS v 0` | many | yes |

So the two most common real commands, `GS V 65` and `ESC !`, are exactly the ones we miss. Every
`escpos-php` stream ends with `GS V 65 n`.

## 3. The gaps, grouped

Priority 1 means a real receipt decodes wrongly today. Priority 2 means it decodes, but a renderer
could not draw it faithfully. Priority 3 is rare or needs a bigger design.

### Priority 1: decodes wrongly

1. **Cut with feed**: `GS V 65 n`, `GS V 66 n`. Also the old `ESC i` and `ESC m`.
2. **Print mode**: `ESC ! n`. Font in bit 0, bold in bit 3, double height in bit 4, double width in
   bit 5, underline in bit 7. It overrides `ESC E`, `ESC M` and `ESC -` in one go.
3. **Graphics**: `GS ( L pL pH m fn ...`. Function 112 stores a raster picture (`a`, `bx`, `by`, `c`,
   `xL xH yL yH`, then rows of `ceil(width/8)` bytes), function 113 stores a column picture,
   function 50 prints what was stored. `GS 8 L` is the same with a four byte length. The other
   functions (48 to 52 and 64 to 69, 80 to 85) are the stored logos in the printer's memory.
4. **Length aware skipping for `FS`**: `FS (` has a length, `FS -`, `FS S`, `FS C` have one byte,
   `FS &` turns on double byte characters. Today the parameters leak out as text.
5. **Control bytes**: `HT`, `BS`, `FF`, `BEL`, `EOT`, `CAN` and `NUL` are control codes, not unknown
   commands. `HT` also moves the print position, so it matters for columns.
6. **Memory safety**: a picture header can promise 4 GB. The decoder then waits and keeps growing
   its buffer. It needs a limit, after which it reports the command as unknown and moves on.
7. **Small bugs found while reading our own decoder**:
   `GS !` should mask each size to 3 bits, so sizes above 8 cannot happen (we accept up to 16).
   `ESC M` should allow fonts C, D and E, we keep only one bit.
   A QR model byte of 51 means Micro QR, we read it as model 2.
   A barcode's text is read as UTF-8, but Code 128 set C holds pairs of digits as raw byte values,
   so the event needs the raw bytes as well as the text.

### Priority 2: decodes, but a renderer needs more

8. **Position and area**: `ESC $ nL nH` (absolute), `ESC \ nL nH` (relative), `GS L` (left margin),
   `GS W` (print area width), `ESC SP n` (character spacing), `ESC D` (tab stops), `GS P` (motion
   units, which scale all of these).
9. **Line spacing and feeding**: `ESC 2`, `ESC 3 n` (kept now, ignored), `ESC + n`, `ESC A n`,
   `ESC J n` (feed n dots), `ESC e n` (reverse feed), `ESC K`, `FF`.
10. **More text state**: `ESC {` (upside down), `ESC V` (rotate 90 degrees), `ESC G` (double
    strike), `ESC r` (second colour), `GS b` (smoothing), `GS |` (density), `ESC 4` and `ESC 5`
    (italic, vendor specific), `ESC R` (international character set, swaps about a dozen ASCII
    characters).
11. **Scaled raster**: `GS v 0 m` where m is 0 to 3 or 48 to 51, which doubles width and height.
12. **Downloaded and stored images**: `GS *` (define), `GS /` (print), `FS q` (store a logo),
    `FS p n m` (print it). These need printer memory that survives between receipts.
13. **User defined characters**: `ESC &` (define), `ESC %` (enable), `ESC ?` (cancel). Parse and
    store now, draw later.
14. **PDF417 settings** (columns, rows, module size, error level) and **QR size** are read and thrown
    away. They should go into the event so a renderer can draw the same symbol.
15. **More barcodes**: `GS k` with m of 74 to 78 (GS1-128 and the GS1 DataBar family), and the
    Code 128 code sets `{A`, `{B`, `{C` and `{1`, `{2`, `{3`.
16. **More 2D symbols** with `GS ( k`: MaxiCode, GS1 DataBar, Composite, Aztec, DataMatrix. The
    emulators disagree on the symbol numbers (one uses 54 for DataMatrix, another 55 for Aztec),
    so check the numbers in the Epson reference before writing them.

### Priority 3: bigger designs

17. **Code page numbers depend on the printer.** See section 4. This one changes the architecture.
18. **Multi byte text**: Shift JIS, GBK, Big5, EUC-KR for `FS &` and `FS C`. `encoding_rs` already
    has them.
19. **Status replies**: `DLE EOT 1..4`, `DLE ENQ`, `DLE DC4`, `GS r 1/2`, `GS I n`, `GS a` (automatic
    status back, 4 bytes), `ESC u`, `ESC v`, `GS ( H`. Apps like CUPS drivers and POS software wait
    for these answers and hang without them. The decoder needs a second output, bytes to send back,
    and a printer state (online, paper, cover, drawer, error) that the API can change.
20. **Page mode**: `ESC L`, `ESC S`, `ESC T`, `ESC W`, `GS $`, `GS \`, `FF`, `CAN`. A mode where the
    page is laid out in a box and printed at once. Rare in receipts.
21. **Star command sets** (StarLine, StarPRNT, `ESC GS t`). None of the eight projects has a Star
    decoder, so we need the Star manuals when we get there.
22. **Arabic shaping**: some printers join Arabic letters in firmware, some do not. This is a
    renderer question (shape the letters when drawing), not a decoder one. Park it until the
    renderer exists.

## 4. Code pages are per printer

Our `codepage.rs` assumes one Epson numbering. The profile database in `escpos-php` and
`python-escpos` (24 printers, same data) shows that is only true for Epson:

| Number | Epson TM-T88IV | Star SP2000 | Zjiang POS-5890 |
|-------:|----------------|-------------|-----------------|
| 1 | CP932 | CP437 | CP932 |
| 2 | CP850 | CP932 | CP850 |
| 4 | CP863 | CP858 | CP863 |
| 16 | CP1252 | CP851 | CP1252 |
| 28 | not present | not present | CP864 |
| 32 | not present | CP1252 | CP1255 |
| 50 | not present | not present | CP437 |

The same number can be a different table, and some printers have 50 tables while others have 11.
An Arabic receipt printed with the wrong table is wrong on the paper, so the emulator has to
choose the right one.

Plan: **printer profiles as data.** A profile holds the code page map (number to name), font sizes
and columns, paper width in dots, and feature flags (graphics, QR, PDF417, Star commands). The
database already has 24 of them and is MIT licensed, so we copy it into `crates/core/profiles/` with
its notice. Each listening port picks a profile. The default is a generic Epson one, so nothing
changes for people who do not choose.

## 5. How we test it

Today every fixture is written by one encoder library, so a bug that library shares with our decoder
would go unseen. Three new sources, all checked the same way as now (expected results come from the
spec, not from any library):

1. **Captured streams from other people's encoders.** Copy the small MIT ones into
   `fixtures/captured/` with a `NOTICE.md`: the `escpos-php` outputs (barcode, qr-code, pdf417-code,
   text-size, margins-and-spacing, graphics, receipt-with-logo, character-tables, character-encodings,
   bit-image, unifont-print-buffer), the `escpresso` raw streams, and the `escpos-tools` receipt.
   About 150 KB. Two checks need no hand written expectation and run on all of them:
   *no unknown events*, and *the same events at every cut position*. A new gap then shows up as a
   failing test the day it is found. Where we do write expectations, we take them from the example
   scripts that made the stream (for example `qr-code.php` says what each code contains).
2. **A second encoder** for the existing `steps` specs. `python-escpos` (and optionally
   `escpos-php`) writes the same specs, and the *same* expected results must hold. Two encoders that
   write different bytes for the same steps, both decoding to the same events, is the independence we
   lack. Needs Python with `venv`. PHP is not installed here, so for PHP we only use the captured
   outputs above.
3. **Hand written byte specs** (kind `bytes`) for every command no encoder writes, such as `ESC $`,
   `GS L`, `ESC V`, status requests. Written from the Epson reference, with out of range values and
   half received commands. Same mutation check as before: break the decoder on purpose and see a
   test fail.

Also: a small seeded generator in the tests that builds random *valid* command sequences with a
known result, next to the random bytes test we already have. No new dependency.

## 6. Stages

One branch, one commit per step, tests before or with each command. The numbers match section 3.

| Stage | What | Closes |
|-------|------|--------|
| A | **Done.** Captured streams and their checks (no unknown commands except a listed gap, no byte left waiting, same events at every cut position). Fixed: cut with feed, `ESC !`, control bytes, `FS` and `DLE` skipping, length framed commands skipped whole, size limits, font numbers, `GS !` sizes, Micro QR. Left from 7: the Code 128 raw data goes with stage D | 1 to 7 |
| B | **Done.** Position, margin, print width, tab and line spacing events, and the style fields: upside down, rotate, double strike, colour, smoothing. Italic is not ESC/POS, so `ESC 4` and `ESC 5` stay unknown | 8 to 10 |
| C | **Done.** `GS ( L` and `GS 8 L`, scaled `GS v 0`, downloaded and stored pictures, and the printer memory with its size and key list replies | 3, 11, 12 |
| D | **Done**, except the size reply of function 82, which needs an encoder. Barcodes, raw Code 128 data, Micro QR, PDF417 settings and the other 2D symbols | 14 to 16 |
| E | **Done.** Printer profiles, per printer code pages, two byte text in JIS, Shift JIS and the Chinese and Korean encodings, UTF-8, `ESC R`, drawn characters and Kanji | 13, 17, 18 |
| F | **Done.** Replies and printer state | 19 |
| G | **Done.** Specs built with python-escpos | section 5 |
| H | Page mode events are **done**; drawing a page belongs to the renderer. Star dialects are still to do | 20, 21 |

After this, the order we agreed stays: renderer (events to a picture), server crate (TCP, REST,
events stream), web page, Tauri shell.

Each stage ends with `docs/ESCPOS.md` updated, the "Not read yet" list shrunk, and the commands to
run (`cargo test --workspace`) given to you instead of run here.

## 7. Decisions needed

1. **Copy the MIT captured streams into the repo?** About 150 KB with a notice. Without them we have
   no independent real data. Recommended: yes.
2. **Second encoder in Python** (`python3 -m venv`, then `pip install python-escpos`)? Here `python3`
   has `venv` but no `pip`, so it may need `sudo apt install python3-venv` first. Recommended: yes.
3. **Copy the printer profile database** (MIT, with notice) as our profile data? Recommended: yes.
4. **Licence for this repo.** Still undecided. MIT would match everything we copy from.
5. **Start with stage A?** It is the one that fixes decoding of real streams.
