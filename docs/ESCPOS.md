# ESC/POS coverage

What `crates/core` decodes. A command that is not here is reported as an unknown event, with its
position and bytes, and decoding goes on. A real printer ignores unknown commands silently, so the
report is only a hint.

The command definitions come from the [Epson ESC/POS command
reference](https://download4.epson.biz/sec_pubs/pos/reference_en/escpos/index.html), as copied in the
manuals of several printer makers, and from the streams that real encoders write. This page says
what the decoder does with each command, in our own words, and names the commands as the reference
does, so a reading can be checked there. The fixtures in `fixtures/specs`
check each command against those definitions. `fixtures/captured` holds streams written by other
people's encoders, which must decode without a single unknown command.

## Text and style

| Command | Bytes | Result |
|---------|-------|--------|
| Initialize | `ESC @` | `Initialize`, and every setting returns to its default |
| Print mode | `ESC ! n` | `Font` and text style: font, bold, double height, double width, underline |
| Bold | `ESC E n` | text style |
| Double strike | `ESC G n` | text style |
| Underline | `ESC - n` | text style: off, thin or thick. Other values are unknown |
| Size | `GS ! n` | text style: width and height from 1 to 8 times. A value with bits 3 or 7 set is ignored, as a printer does, and reported as unknown |
| Invert | `GS B n` | text style |
| Smoothing | `GS b n` | text style |
| Font | `ESC M n` | `Font` and text style: fonts A to E, as 0 to 4 or the digits `0` to `4` |
| Character spacing | `ESC SP n` | text style: space to the right of each character, in motion units |
| Rotation | `ESC V n` | text style: a quarter turn clockwise |
| Upside down | `ESC { n` | text style |
| Second colour | `ESC r n` | text style |
| Code page | `ESC t n` | how the following text bytes are read, by the printer profile |
| International set | `ESC R n` | twelve ASCII characters read as the characters of a country, for 0 to 15 |
| Drawn characters | `ESC & y c1 c2 ...`, `ESC % n`, `ESC ? n` | `DefineCharacter`, `UserCharacters`, `DeleteCharacter`. The text shows a drawn character as a private use character, see `UserCharacter::glyph` |
| Two byte characters | `FS &`, `FS .` | text is read in pairs of bytes, as the encoding of the printer says, while they are on |
| Kanji code system | `FS C n` | 0 is JIS code, which is read until it is changed, and 1 and 2 are Shift JIS, in which the lead byte of a character starts one even with `FS &` off. Only a Japanese model, which has Shift JIS in its profile, has the choice. Other values are unknown |
| Kanji font | `FS ( A` with fn 48 | 0 to 2: font A (24 by 24 dots), B (20 by 24), C (16 by 16). It decides how many bytes `FS 2` takes |
| Drawn Kanji | `FS 2 c1 c2 d1 ... dk` | `DefineKanji`, with k of 72, 60 or 32 by the font. The text shows a drawn Kanji as a private use character, see `UserKanji::glyph`, and a code that may be drawn and was not as a space. `ESC @` forgets them |
| UTF-8 text | `FS ( C` with fn 48 and m of 1 or 2 | text is read as UTF-8 until m is 0 |
| Tab | `HT` | stays in the text as a tab character |
| Tab stops | `ESC D n1 ... nk NUL` | `TabStops`. They go up, there are at most 32, and a byte that breaks the order ends the list and is read as text |

## Layout

| Command | Bytes | Result |
|---------|-------|--------|
| Align | `ESC a n` | `Align`, applied to the text after it |
| Absolute position | `ESC $ nL nH` | `Position::Absolute`, in motion units |
| Relative position | `ESC \ nL nH` | `Position::Relative`, a negative move is the complement of 65536 |
| Left margin | `GS L nL nH` | `Margin` |
| Print area width | `GS W nL nH` | `PrintWidth` |
| Motion units | `GS P x y` | `MotionUnits`: a horizontal unit is 1/x inch and a vertical one 1/y inch |
| Line spacing | `ESC 2`, `ESC 3 n`, `ESC A n`, `ESC + n` | `LineSpacing` |
| Feed lines | `ESC d n`, `ESC e n` | `Feed` in lines, forward or back |
| Feed in units | `ESC J n`, `ESC K n` | `Feed` in motion units, forward or back |
| Density | `GS \| n` | `Density`, from -500 to 500 tenths of a percent |
| Line feed, carriage return | `LF`, `CR` | ends a line; a carriage return is ignored |

Positions, margins and the print area are in motion units, which only a printer profile and a
`GS P` can turn into dots, so the decoder reports them as they were sent.

## Pictures

| Command | Bytes | Result |
|---------|-------|--------|
| Raster picture | `GS v 0 m xL xH yL yH d...` | `Image`. Modes 0 to 3 and the digits: double width and double height repeat the dots |
| Column picture | `ESC * m nL nH d...` | `Image`, modes 0, 1, 32 and 33. The 8 dot modes are repeated 3 times down and the single density modes 2 times across, as the printer prints them |
| Downloaded picture | `GS * x y d...`, `GS / m` | `Image` when printed. Defining a character takes it back |
| Pictures in memory | `FS q n ...`, `FS p n m` | `Image` when printed. `ESC @` does not clear them |
| Graphics | `GS ( L` and `GS 8 L`, functions 112 and 113 (store raster and column), 50 (print) | `Image`. A second colour is laid over the first |
| Graphics in memory | functions 67 and 68 (define in the kept memory), 69 (print), 66 (delete one), 65 (delete all) | `Image` when printed |
| Downloaded graphics | functions 83 and 84 (define), 85 (print), 82 and 81 (delete) | `Image` when printed |

The functions 0 to 4 are other numbers for 48 to 52, and are read as them.

| Function | Result |
|----------|--------|
| 49 | `GraphicsDensity`: 180 or 360 dots per inch, from x and y of 50 or 51. Other values are unknown |
| 48, 51, 52 | The whole size of the NV memory, what is left of it, and what is left of the download memory, as `37 id digits 00`. The sizes are 256 KiB and 12 KiB, as an Epson TM-T88 has by default, and only the bytes of the dots count as taken |
| 64, 80 | The keys of the graphics in the NV or the download memory, as `37 72` or `37 73`, a status of 0x40 or 0x41, the key pairs and `00`. At most 40 keys go in one answer. While more follow, the program answers with ACK for the next block, NAK for the same one again and CAN to stop. After the last, ACK ends it. Any other byte ends it and is read as usual |

Pictures are given in printer dots.

## Barcodes and two dimensional codes

| Command | Bytes | Result |
|---------|-------|--------|
| Barcode settings | `GS h`, `GS w`, `GS H`, `GS f` | apply to the next barcode. A value that is not allowed is unknown |
| Barcode | `GS k` | `Barcode`: UPC-A, UPC-E, EAN-13, EAN-8, Code 39, ITF, Codabar, Code 93, Code 128 in both command forms, and GS1-128 and the four GS1 DataBar forms in the form with a length. Code 128 data follows the character sets: set C pairs are written as digits, and `{{` is a brace |
| QR code | `GS ( k`, symbol 49 | `Qr`: model 1, 2 or 3 (Micro QR), size, error level, data |
| PDF417 | `GS ( k`, symbol 48 | `Pdf417`: data, columns, rows, module size, error level and whether it is truncated |
| Other codes | `GS ( k`, symbols 50 to 54 | `Symbol`: MaxiCode, GS1 DataBar, Composite, Aztec and DataMatrix, with the raw settings |

The request for the size of a symbol, function 82, is read and not answered: the size in dots is the
size of the encoded symbol, and only an encoder knows it.

## Cuts, drawer and buzzer

| Command | Bytes | Result |
|---------|-------|--------|
| Cut | `GS V m` | `Cut`, full or partial, for m of 0, 1, 48 and 49 |
| Cut with feed | `GS V m n` | `Cut` for m of 65, 97 and 103 (full) and 66, 98 and 104 (partial). The feed length is read and ignored |
| Older cuts | `ESC i`, `ESC m` | `Cut`, partial |
| Cash drawer | `ESC p m t1 t2`, `DLE DC4 1 m t` | `Pulse`, in milliseconds |
| Buzzer | `BEL`, `ESC B n t`, `ESC ( A` | `Beep` |

## Page mode

`ESC L` enters page mode and `ESC S` or `FF` leaves it. `ESC T` sets the direction, `ESC W` the page,
`GS $` and `GS \` move in the other direction, `ESC FF` prints and `CAN` cancels. They are reported
as `Page` events in order. Drawing a page is for whoever draws the events.

## What the printer says

A program can ask the printer things. The answers wait in `Decoder::take_replies`, or
`Printer::take_replies`, for the connection to send back. What the printer says comes from a
`PrinterStatus`, which a program or a person can change with `set_status`.

| Request | Answer |
|---------|--------|
| `DLE EOT n` for n of 1 to 4 | one byte: the printer, the off line cause, the error cause, or the roll paper sensor |
| `DLE ENQ n` | nothing, but a fault that can be recovered from goes away |
| `GS r n` for n of 1, 2, 49 and 50 | one byte: the paper sensor or the cash drawer |
| `GS I n` for n of 1 to 3 and 65 to 68 | the model, type and version bytes, or text: the version, maker, model name and serial number |
| `GS a n` | the four bytes of automatic status back at once when n is not 0, and again whenever a status that n asked for changes |
| `GS ( H` with fn 48 | `37 22`, the four bytes it was given, and a NUL |

A request with a value that is not allowed is not answered and is reported as unknown.

## Settings that change nothing that is drawn

These are read to their end and ignored, so their parameters do not print as text: `ESC =`, `ESC U`,
`ESC c`, `ESC <`, `GS ( A`, `GS ( C`, `GS ( D`, `GS ( E`, `GS ( K`, `GS ( M`, `GS ( N`, `FS ( A`,
`FS ( E`, `FS ( L`, `FS !`, `FS -`, `FS C`, `FS W`, `FS S`.

## How a stream is read

- Bytes can arrive in any number of pieces. A half received command waits for the rest, and the
  result does not depend on where the stream was cut.
- Text is collected until a line feed, a style change, or any command that prints, then read in the
  current code page.
- A receipt ends at a cut, or when the stream ends with something unprinted. Blank lines and
  carriage returns after a cut print nothing, so they are not a receipt.
- An unknown command is reported once, together with the control bytes that follow it. When the
  length of a command is known, the whole command is reported, so its data is never read as text.

## Limits

So that a bad stream cannot make the decoder wait or grow without end: a command may take at most
8 MiB, the data of a barcode may run 4096 bytes before its end byte, text with no line feed is
reported in pieces of 4096 bytes, and the pictures the printer keeps in memory may take 32 MiB.
Past those limits the command is reported as unknown.

## Code pages and printers

What `ESC t n` selects depends on the printer. `Decoder::with_profile` takes one of the 24 printer
profiles of `Profile::named`, or the generic one, and the profile gives the code page for each
number. A number that the profile does not have is read as Windows 1252.

The pages are read with tables for the IBM pages 437, 720, 737, 775, 850, 852, 855, 857, 858, 860
to 866, 869 and 874, the Windows pages 1250 to 1258 and the ISO 8859 pages. A few have no table
and are read as the closest page: CP851, CP853, CP856, CP1098, CP1125, CP772, CP774, RK1048 and
the Vietnamese pages. Pages that only one maker has, such as Star's CP3012, are read as
Windows 1252.

The international character sets 0 to 15 are written from the Epson table. Sets 14 and 15 are the
least checked.

## Not read yet

- The Star command sets, StarLine and StarPRNT. A Star printer in ESC/POS mode works.
- The size of a two dimensional code, function 82 of `GS ( k`, which an encoder has to tell.
- Drawing a page of page mode. The decoder reports its events in order, and whoever draws them
  puts them in the page.

`ESC 4`, `ESC 5` and `ESC F` are not ESC/POS commands. They belong to the dot matrix command sets
of Epson, where they mean italics and bold, and a printer of the ESC/POS family ignores them, so
they are reported as unknown here.
