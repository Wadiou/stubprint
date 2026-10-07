"""A picture of what a stream prints, for browsing the fixtures: the printed picture, the text
lines, then every barcode and 2D code, which the printer would draw itself. The codes are drawn
by libraries, so they show what is encoded, not how a particular printer renders it. The tests do
not read these."""
import barcode
import pdf417gen
import qrcode
from barcode.writer import ImageWriter
from PIL import Image, ImageDraw

from shaping import FONTS, FILES, draw_line, font

# What python-barcode can draw. Codabar, Code 93 and UPC-E have no picture here.
BARCODE_NAMES = {"upca": "upca", "ean13": "ean13", "ean8": "ean8", "code39": "code39", "itf": "itf", "code128": "code128"}
QR_LEVELS = {"L": qrcode.constants.ERROR_CORRECT_L, "M": qrcode.constants.ERROR_CORRECT_M, "Q": qrcode.constants.ERROR_CORRECT_Q, "H": qrcode.constants.ERROR_CORRECT_H}
MONO_FILE = str(FONTS / FILES[("latin", False)])
DPI = 203


def _note(text):
    """A small picture with a sentence, for a code that cannot be drawn."""
    image = Image.new("L", (360, 28), 255)
    draw_line(ImageDraw.Draw(image), text, 4, 20, "left", 14)
    return image


def _barcode_image(barcode_spec):
    name = BARCODE_NAMES.get(barcode_spec["symbology"])
    if name is None:
        return _note(f"no preview for {barcode_spec['symbology']}")
    try:
        options = {"add_checksum": False} if name == "code39" else {}
        code = barcode.get(name, barcode_spec["data"], writer=ImageWriter(), **options)
        dots = barcode_spec.get("width", 2)
        return code.render(
            {
                "dpi": DPI,
                "module_width": dots * 25.4 / DPI,
                "module_height": barcode_spec.get("height", 60) * 25.4 / DPI,
                "quiet_zone": 2,
                "font_path": MONO_FILE,
                "font_size": 8,
                "write_text": barcode_spec.get("text", "below") != "none",
            }
        ).convert("L")
    except Exception as error:  # a preview must never stop the generator
        return _note(f"no preview: {error}"[:60])


def _code_images(expected):
    codes = []
    for spec in expected.get("barcodes", []):
        codes.append((f"{spec['symbology']}  {spec['data']}", _barcode_image(spec)))
    for spec in expected.get("qrs", []):
        qr = qrcode.QRCode(error_correction=QR_LEVELS[spec["level"]], box_size=max(2, spec["size"]), border=2)
        qr.add_data(spec["data"])
        qr.make(fit=True)
        codes.append((f"qr model {spec['model']}, level {spec['level']}, size {spec['size']}  {spec['data']}", qr.make_image().get_image().convert("L")))
    for spec in expected.get("pdf417", []):
        codes.append((f"pdf417  {spec['data']}", _pdf417_image(spec["data"])))
    return codes


def _pdf417_image(data):
    """pdf417gen needs at least 3 rows, so a short text needs fewer columns than its default of 6."""
    try:
        for columns in range(6, 0, -1):
            try:
                codes = pdf417gen.encode(data, columns=columns)
            except ValueError:
                continue
            return pdf417gen.render_image(codes, scale=3, ratio=3, padding=4).convert("L")
        return _note("no preview: too short for pdf417gen")
    except Exception as error:  # a preview must never stop the generator
        return _note(f"no preview: {error}"[:60])


def _kanji_index(code):
    """Where a Kanji a program drew is in the private use area: the decoder's UserKanji::glyph."""
    first, second = code
    if first == 0x77 and 0x21 <= second <= 0x7E:
        return second - 0x21
    if first == 0xEC and 0x40 <= second <= 0x7E:
        return second - 0x40
    if first == 0xEC and 0x80 <= second <= 0x9E:
        return second - 0x80 + 63
    if first == 0xFE and 0xA1 <= second <= 0xFE:
        return second - 0xA1
    return None


def _drawn_characters(expected):
    """The characters a program drew, by the private use character the decoder shows for each."""
    drawn = {}
    for event in expected.get("userCharacters", []):
        if "define" in event:
            d = event["define"]
            data = bytes.fromhex(d["data"])
            size = d["columnBytes"]
            drawn[chr(0xE000 + d["code"])] = {
                "width": d["width"],
                "height": size * 8,
                "columns": [data[i * size : (i + 1) * size] for i in range(d["width"])],
            }
        # A deleted character stays here: a line printed before the delete still shows it.
    for event in expected.get("userKanji", []):
        index = _kanji_index(bytes.fromhex(event["code"]))
        data = bytes.fromhex(event["data"])
        size = event["height"] // 8
        drawn[chr(0xE100 + index)] = {
            "width": event["width"],
            "height": event["height"],
            "columns": [data[i * size : (i + 1) * size] for i in range(event["width"])],
        }
    return drawn


def render_preview(expected, bitmap_image, lines):
    """Returns a picture, or None when a stream has nothing to show."""
    codes = _code_images(expected)
    texts = lines if lines is not None else expected.get("texts", [])
    if bitmap_image is None and not codes and not texts:
        return None

    columns = expected.get("columns")
    width = max(expected["image"]["width"] if "image" in expected else 0, columns * 12 if columns else 0, 384)
    pieces = []
    y = 8
    if bitmap_image is not None:
        pieces.append((bitmap_image, (width - bitmap_image.width) // 2, y))
        y += bitmap_image.height + 8
    text_rows = []
    for line in texts:
        y += 24
        text_rows.append((line, y))
        y += 4
    if texts:
        y += 8
    captions = []
    for caption, image in codes:
        y += 18
        captions.append((caption, y))
        y += 6
        pieces.append((image, max(0, (width - image.width) // 2), y))
        y += image.height + 10

    canvas = Image.new("L", (width, y + 8), 255)
    draw = ImageDraw.Draw(canvas)
    for image, x, top in pieces:
        canvas.paste(image, (x, top))
    drawn = _drawn_characters(expected)
    for line, baseline in text_rows:
        draw_line(draw, line, 12, baseline, "left", 20, drawn=drawn)
    for caption, baseline in captions:
        draw.text((12, baseline), caption, font=font("latin", False, 14), fill=100, anchor="ls")
    return canvas
