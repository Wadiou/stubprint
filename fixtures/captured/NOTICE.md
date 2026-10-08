# Captured streams

Byte streams written by other people's encoders, copied here so the decoder is tested on data we
did not make. They are used as they are: never edited, never regenerated.

| Folder | From | Licence | What it is |
|--------|------|---------|------------|
| `escpos-php/` | [mike42/escpos-php](https://github.com/mike42/escpos-php), `test/integration/resources/output` | MIT, see `escpos-php/LICENSE.md` | Output of the library's example scripts: barcodes, QR, PDF417, text sizes, margins, graphics, a receipt with a logo, code pages |
| `escpresso/` | [jflaflamme/escpresso](https://github.com/jflaflamme/escpresso), `tests/raw` | MIT, see `escpresso/LICENSE` | Streams from a CUPS receipt driver: formats, a receipt, spacing, code pages |

`scripts/upstream.py` copies them from the releases that `upstream/lock.json` names, and CI checks
their hashes. The licence texts above must stay next to the files. What each stream is meant to contain is taken
from the example scripts that made it (for `escpos-php`, the `example/` folder of that project), not
from any decoder.
