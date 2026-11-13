# Star Line mode and StarPRNT coverage

What `crates/core` decodes when a decoder is set to `Dialect::StarLine` or `Dialect::StarPrnt`. As in
[ESC/POS](ESCPOS.md), a command that is not here is reported as an unknown event with its position
and bytes, and decoding goes on, and bytes can arrive in any number of pieces.

The commands come from the Star manuals: the
[Line Mode command specifications](https://www.starmicronics.com/support/Mannualfolder/starline_cm_en.pdf)
and the [StarPRNT command specifications](https://www.starmicronics.com/support/Mannualfolder/StarPRNT_cm_en.pdf).
The fixtures in `fixtures/specs` named `starline-*` and `starprnt-*` check each command against them.
Nothing else writes these languages, so the expected values are written from the manuals, with Python's
own tables for the code pages and the packed data of the manual's example for the raster commands.

A Star printer reads one language, and the same byte means other things in each: ENQ is a request in
Line mode and nothing in ESC/POS and StarPRNT, `GS` alone is nothing in Star, and the commands start
with `ESC GS` where those of ESC/POS start with `GS`. A decoder is set to one language, like the
emulation setting of a printer, and a port will be too.

Distances are in dots, at 8 to the millimetre.

## Text and style

| Command | Result |
|---------|--------|
| `ESC RS F n` | `Font`: 0 is font A, 1 is font B, 16 is the OCR font (font 2) |
| `ESC GS t n` | the code page, by the numbers of the manual. 128 reads UTF-8 |
| `ESC GS ) U` and `ESC ) U`, function 48 | UTF-8 reading on and off |
| `ESC R n` | the international set, 0 to 13, or as digits and letters. Ireland and the legal set have no table here |
| `ESC SP n`, and `ESC M`, `ESC g`, `ESC P`, `ESC :` | the room right of each character, 0 to 15 dots. The four pitches are Line mode only |
| `ESC i n1 n2`, `ESC W n`, `ESC h n`, and `SO`, `DC4`, `ESC SO`, `ESC DC4` | height and width, from 1 to 6 times. The last four are Line mode only |
| `ESC E`, `ESC F`, `ESC - n`, `ESC _ n`, `ESC 4`, `ESC 5`, `SI`, `DC2`, `ESC GS b n` | bold, underline, a line over (`overline`), white on black, upside down, smoothing |
| `ESC & c1 c2 n d...`, `ESC %` | a character of 12 by 24 dots in rows, put to use or not: `DefineCharacter`, `DeleteCharacter`, `UserCharacters` |
| `ESC p`, `ESC q`, `ESC $ n`, `ESC r c1 c2 d...` | Kanji in JIS code or Shift JIS, and a drawn Kanji of 24 by 24 dots: `DefineKanji` |

## Layout

| Command | Result |
|---------|--------|
| `ESC a n`, `ESC J n`, `ESC I n`, `FF` | `Feed` of lines, of n quarters or eighths of a millimetre, or to the next page |
| `ESC z n`, `ESC 0` | `LineSpacing` of 3 or 4 millimetres |
| `ESC l n`, `ESC Q n` | `Margin` and `PrintWidth`, in characters of the pitch and width of the moment |
| `ESC D n1 ... NUL`, `HT` | `TabStops`, at most 16, and the tab in the text |
| `ESC GS A`, `ESC GS R`, `ESC GS a` | `Position` and `Align` |
| `ESC d n` | `Cut`. 2 and 3 feed to the cutter first, which is not shown |
| `BEL`, `FS`, `SUB`, `EM`, `ESC BEL n1 n2` | `Pulse` of the first or second drawer, 200 ms unless told |
| `ESC GS BEL`, `ESC GS EM DC1` and `DC2` | `Beep` |
| `ESC GS EM DC3` and `DC4` | the four drawer terminals of StarPRNT |
| `ESC C`, `ESC B`, `VT` | the page length and the tabs down the page, read and not used |

## Pictures

| Command | Result |
|---------|--------|
| `ESC K`, `ESC L`, `ESC k`, `ESC X` | `Image`: each dot 3 by 3, 1 by 3, 1 by 1, or a column of 24 |
| `ESC FS q`, `ESC FS p`, `ESC RS L` | logos, as `FS q` and `FS p` of ESC/POS, and all or none of them at once. Line mode only |
| `ESC GS ( L`, `ESC GS 8 L` | the graphics of ESC/POS, in the memory with its sizes and key lists |
| `ESC * r A` ... `ESC * r B` | raster mode, Line mode only: `b` adds a row, `k` adds to a row, `ESC * r Y` moves down, `ESC FF EOT` ends the document with the cut that `ESC * r E` says. They make one `Image` |
| `ESC GS S`, `ESC GS X` | StarPRNT raster graphics, and the same packed |

## Codes

| Command | Result |
|---------|--------|
| `ESC b n1 n2 n3 n4 d... RS` | `Barcode`. The kinds and the modes that give the width of the bars are the manual's. Code 128 and Code 93 data uses `%` for what cannot be sent |
| `ESC GS y` | `Qr`: model, level, cell, and data by `D 1` or in blocks by `D 2` |
| `ESC GS x` | `Pdf417`: the size by rows and columns or by proportion, the level, the module and the data |
| `ESC GS ( k`, symbols 51 and 52 | `Symbol`, as in ESC/POS |

The requests `ESC GS x I` and `ESC GS y I` ask whether a code can be printed and how big it is, which
only an encoder knows, so they are read and not answered.

## Page mode

`ESC GS P 0` to `8` give the same `Page` events as page mode in ESC/POS: enter, leave, the direction, the
region, the place down the page, print, print and leave, and throw away.

## What the printer says

| Request | Answer |
|---------|--------|
| `ENQ`, `EOT` (Line mode) | one byte: paper end and another error, or paper end and near end |
| `ESC ACK SOH` | the nine bytes of the automatic status (ASB) |
| `ESC RS a n` | 1 and 3 have the status sent by itself whenever it changes. 255 sends it now |
| `ETB`, `ESC RS E` | count a point of the job in the status, and clear the counter |
| `ESC GS ETX s n1 n2` | the print end counter, and the time to wait for data |
| `ESC # * LF NUL` | the model and the version, which is 1.0 |
| `ESC ACK CAN`, `ESC ? LF NUL`, `CAN`, `ESC @` | reset the printer, or the commands |

## Read and left alone

Density, speed, colour, the top margin, the print area, the memory switches, the auto logo, presenters,
information with a length (`ESC GS ) x`, `ESC GS J`), text search, and the turning and shrinking of a
page. They do not change what is drawn.

## Not read yet

- Marks (`ESC GS *`), audio (`ESC GS s`), and the commands of card readers and customer displays,
  which are reported as unknown.
- Two colours: the commands are read and the red is not drawn.
- The sizes that `ESC GS x I` and `ESC GS y I` answer.
- Drawing a page of page mode, which the renderer will do.
