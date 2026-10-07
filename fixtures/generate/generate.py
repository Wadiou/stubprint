"""Builds the printer streams the decoder tests read, from the recipes in fixtures/specs/.

    python generate.py                 write fixtures/out/ and check it against fixtures.lock.json
    python generate.py --update-lock   the same, then accept the result as the new lock

The streams are written by python-escpos, a library people use to print on real printers. The
outputs are not committed. The lock holds a hash of each one, so a library, font or spec change
that moves a single dot is noticed instead of silently changing what the tests check. The expected
results come from what a spec asks for, not from reading the library's output.
"""
import contextlib
import hashlib
import io
import json
import math
import shutil
import sys
from pathlib import Path

from escpos import constants as escpos_constants
from escpos.printer import Dummy
from PIL import Image

from data import make_receipt
from layout_image import draw_receipt, pack_bits
from layout_text import text_lines
from preview import render_preview
from shaping import require_raqm

HERE = Path(__file__).resolve().parent
SPECS = HERE.parent / "specs"
OUT_ROOT = HERE.parent / "out"
OUT = OUT_ROOT / "escpos"
PREVIEWS = OUT_ROOT / "previews"
LOCK = HERE.parent / "fixtures.lock.json"
UPDATE_LOCK = "--update-lock" in sys.argv

# The names a spec uses, and what python-escpos and Python call them.
CODEPAGES = {"cp437": ("CP437", "cp437"), "windows1252": ("CP1252", "cp1252"), "windows1256": ("CP1256", "cp1256")}
SYMBOLOGIES = {"upca": "UPCA", "upce": "UPCE", "ean13": "EAN13", "ean8": "EAN8", "code39": "CODE39", "itf": "ITF", "codabar": "CODABAR", "code93": "CODE93", "code128": "CODE128"}
TEXT_POSITIONS = {"none": "OFF", "above": "ABOVE", "below": "BELOW", "both": "BOTH"}
QR_LEVELS = {"l": escpos_constants.QR_ECLEVEL_L, "m": escpos_constants.QR_ECLEVEL_M, "q": escpos_constants.QR_ECLEVEL_Q, "h": escpos_constants.QR_ECLEVEL_H}
QR_MODELS = {1: escpos_constants.QR_MODEL_1, 2: escpos_constants.QR_MODEL_2}
RULE_COLUMNS = 48
FRAGMENT_HEIGHT = 960  # python-escpos splits a taller picture into commands of this many rows


class SpecError(Exception):
    pass


def fail(spec, message):
    raise SpecError(f"spec {spec['name']}: {message}")


def new_printer():
    return Dummy(profile="default")


def columns_for(width):
    return 32 if width == 384 else 48


def dots_for(paper):
    return 384 if paper == 58 else 576


def order_url(receipt):
    return f"https://example.com/orders/{receipt['code']}"


def check_codepage(spec, text, codepage):
    """Proves the text survives the code page, so a spec cannot ask for a letter it cannot print."""
    if codepage is None:
        if any(not 0x20 <= ord(c) <= 0x7E for c in text):
            fail(spec, f'"{text}" needs a codepage step before it')
        return
    codec = CODEPAGES[codepage][1]
    try:
        survives = text.encode(codec).decode(codec) == text
    except UnicodeEncodeError:
        survives = False
    if not survives:
        fail(spec, f'"{text}" does not survive {codepage}')


def codes(receipt):
    return {
        "barcodes": [{"symbology": "code128", "data": receipt["code"], "height": 60}],
        "qrs": [{"data": order_url(receipt), "model": 2, "size": 4, "level": "Q"}],
    }


def barcode(printer, value, symbology, height=64, width=3, text="below"):
    """Code 128 data starts with the character set to use, such as {B."""
    if symbology == "code128" and not value.startswith("{"):
        value = "{B" + value
    printer.barcode(value, SYMBOLOGIES[symbology], height=height, width=width, pos=TEXT_POSITIONS[text], align_ct=False)


def qrcode(printer, value, model=2, size=3, level="l"):
    printer.qr(value, ec=QR_LEVELS[level], size=size, model=QR_MODELS[model], native=True)


def pdf417(printer, value):
    """python-escpos has no PDF417, so these are the GS ( k commands for it, written as the Epson
    reference describes them: the model, columns, module size, error level, the data, then print."""
    def command(function, data, parameter=None):
        body = bytes([0x30, function]) + (parameter if parameter is not None else b"") + data
        printer._raw(b"\x1d\x28\x6b" + len(body).to_bytes(2, "little") + body)

    command(0x46, b"\x00")
    command(0x41, b"\x00")
    command(0x43, b"\x03")
    command(0x44, b"\x03")
    command(0x45, b"\x01", b"\x31")
    command(0x50, value.encode("utf-8"), b"\x30")
    command(0x51, b"", b"\x30")


def count(stream, prefix):
    return sum(1 for i in range(len(stream) - len(prefix) + 1) if stream[i : i + len(prefix)] == prefix)


def bits_to_image(bits, width, height):
    """The packed bits as a picture, black on white."""
    row_bytes = (width + 7) // 8
    return Image.frombytes("1", (width, height), bytes(~byte & 0xFF for byte in bits[: row_bytes * height]))


def native(spec):
    """A receipt in the printer's own font, one character per column, in a code page."""
    receipt = make_receipt(spec["receipt"]["lang"])
    width = dots_for(spec["paper"])
    columns = columns_for(width)
    lines = text_lines(receipt, columns)
    printer = new_printer()
    printer.hw("INIT")
    printer.charcode(CODEPAGES[spec["codepage"]][0])
    for text, bold in lines:
        check_codepage(spec, text, spec["codepage"])
        if bold:
            printer.set(bold=True)
        printer.textln(text)
        if bold:
            printer.set(bold=False)
    barcode(printer, receipt["code"], "code128", height=60)
    qrcode(printer, order_url(receipt), model=2, size=4, level="q")
    printer.ln()
    printer.cut()
    return {
        "bytes": printer.output,
        "expected": {
            "receipts": 1,
            "codepage": spec["codepage"],
            "columns": columns,
            "texts": [text for text, _ in lines if text != ""],
            "boldTexts": [text for text, bold in lines if bold],
            **codes(receipt),
            "cuts": ["full"],
        },
        "text": "\n".join(text for text, _ in lines),
    }


def picture(spec):
    """A receipt drawn as a picture, so any language prints. A hybrid adds the printer's own barcode and QR."""
    receipt = make_receipt(spec["receipt"]["lang"])
    image, bits = draw_receipt(receipt, dots_for(spec["paper"]))
    printer = new_printer()
    printer.hw("INIT")
    printer.image(image, impl="bitImageRaster", fragment_height=FRAGMENT_HEIGHT)
    hybrid = spec["mode"] == "hybrid"
    if hybrid:
        barcode(printer, receipt["code"], "code128", height=60)
        qrcode(printer, order_url(receipt), model=2, size=4, level="q")
    printer.ln()
    printer.cut()
    stream = printer.output
    bands = math.ceil(image.height / FRAGMENT_HEIGHT)
    found = count(stream, b"\x1d\x76\x30\x00")
    if found != bands:
        fail(spec, f"expected {bands} raster commands, the stream has {found}")
    return {
        "bytes": stream,
        "bitmap": bits,
        "expected": {
            "receipts": 1,
            "image": {"width": image.width, "height": image.height},
            "rasterBands": bands,
            **(codes(receipt) if hybrid else {}),
            "cuts": ["full"],
        },
    }


def pattern(spec):
    """A frame, two diagonals and, when there is room, a checkerboard. A wrong bit shows as a
    visible flaw. Column mode pads the last strip to 24 rows, raster mode sends the rows as they are."""
    height = spec["height"]
    width = spec.get("width", 384)
    mode = spec.get("imageMode", "raster")
    fragment = spec.get("fragmentHeight", FRAGMENT_HEIGHT)
    padded = math.ceil(height / 24) * 24 if mode == "column" else height

    image = Image.new("L", (width, padded), 255)
    pixels = image.load()
    for y in range(height):
        for x in range(width):
            frame = x < 4 or x >= width - 4 or y < 4 or y >= height - 4
            diagonal = abs(x - y) < 3 or abs(width - x - y) < 3
            checker = width >= 128 and height >= 128 and 40 <= x < 104 and 40 <= y < 104 and ((x >> 3) + (y >> 3)) % 2 == 0
            if frame or diagonal or checker:
                pixels[x, y] = 0
    bits = pack_bits(image)

    printer = new_printer()
    printer.hw("INIT")
    # The padding rows are white, so the picture sent is the one of the real height.
    printer.image(image.crop((0, 0, width, height)), impl="bitImageColumn" if mode == "column" else "bitImageRaster", fragment_height=fragment)
    printer.cut()
    stream = printer.output

    expected = {"receipts": 1, "image": {"width": width, "height": padded}, "cuts": ["full"]}
    if mode == "raster":
        expected["rasterBands"] = math.ceil(height / fragment)
        found = count(stream, b"\x1d\x76\x30\x00")
        if found != expected["rasterBands"]:
            fail(spec, f"expected {expected['rasterBands']} raster commands, the stream has {found}")
    else:
        expected["columnStrips"] = padded // 24
        found = count(stream, b"\x1b\x2a\x21")
        if found != expected["columnStrips"]:
            fail(spec, f"expected {expected['columnStrips']} column strips, the stream has {found}")
    return {"bytes": stream, "bitmap": bits, "expected": expected}


def steps(spec):
    """An ordered list of library calls, for one feature at a time. What the stream must decode
    to is written in the spec, by hand, and is not read back from the library."""
    printer = new_printer()
    codepage = None
    for step in spec["steps"]:
        operation = step["op"]
        if operation == "initialize":
            printer.hw("INIT")
        elif operation == "codepage":
            codepage = step["name"]
            printer.charcode(CODEPAGES[codepage][0])
        elif operation == "font":
            printer.set(font=step["name"])
        elif operation == "align":
            printer.set(align=step["value"])
        elif operation == "bold":
            printer.set(bold=step["value"])
        elif operation == "underline":
            printer.set(underline=1 if step["value"] else 0)
        elif operation == "invert":
            printer.set(invert=step["value"])
        elif operation == "size":
            printer.set(custom_size=True, width=step["width"], height=step["height"])
        elif operation == "line":
            check_codepage(spec, step["text"], codepage)
            printer.textln(step["text"])
        elif operation == "newline":
            printer.ln(step.get("count", 1))
        elif operation == "rule":
            printer.textln(("═" if step["style"] == "double" else "─") * RULE_COLUMNS)
        elif operation == "barcode":
            barcode(printer, step["value"], step["symbology"], step.get("height", 64), step.get("width", 3), step.get("text", "below"))
        elif operation == "qrcode":
            qrcode(printer, step["value"], step.get("model", 2), step.get("size", 3), step.get("level", "l"))
        elif operation == "pdf417":
            pdf417(printer, step["value"])
        elif operation == "pulse":
            if "device" in step:
                printer.cashdraw([0x1B, 0x70, step["device"], step["on"] // 2, step["off"] // 2])
            else:
                printer.cashdraw(2)
        elif operation == "cut":
            printer.cut(mode="FULL" if step["mode"] == "full" else "PART")
        else:
            fail(spec, f'unknown step "{operation}"')
    return {"bytes": printer.output, "expected": spec["expect"]}


def hand_written(spec):
    """Hand written bytes: hex pairs and `text:...`, for cases no encoder would ever produce."""
    out = bytearray()
    for token in spec["bytes"]:
        if token.startswith("text:"):
            text = token[5:]
            if any(not 0x20 <= ord(c) <= 0x7E for c in text):
                fail(spec, f'"{text}" is not printable ASCII')
            out += text.encode("ascii")
        else:
            for pair in token.split():
                if len(pair) != 2 or any(c not in "0123456789abcdefABCDEF" for c in pair):
                    fail(spec, f'"{pair}" is not a hex byte')
                out.append(int(pair, 16))
    return {"bytes": bytes(out), "expected": spec["expect"]}


def build(spec):
    kind = spec["kind"]
    if kind == "receipt":
        if spec["mode"] == "native":
            return native(spec)
        if spec["mode"] in ("picture", "hybrid"):
            return picture(spec)
        fail(spec, f'unknown mode "{spec["mode"]}"')
    if kind == "pattern":
        return pattern(spec)
    if kind == "steps":
        return steps(spec)
    if kind == "bytes":
        return hand_written(spec)
    return fail(spec, f'unknown kind "{kind}"')


def hex_dump(stream):
    """Offset, hex bytes, and the printable characters of a stream."""
    lines = []
    for offset in range(0, len(stream), 16):
        row = stream[offset : offset + 16]
        hex_part = " ".join(f"{byte:02x}" for byte in row)
        text_part = "".join(chr(byte) if 0x20 <= byte <= 0x7E else "." for byte in row)
        lines.append(f"{offset:08x}  {hex_part:<47}  |{text_part}|")
    return "\n".join(lines) + "\n"


def sha256(data):
    return hashlib.sha256(data if isinstance(data, bytes) else data.encode("utf-8")).hexdigest()


def main():
    require_raqm()
    spec_files = sorted(SPECS.glob("*.json"))
    shutil.rmtree(OUT_ROOT, ignore_errors=True)
    OUT.mkdir(parents=True)
    for folder in ("png", "hex", "text"):
        (PREVIEWS / folder).mkdir(parents=True)

    hashes = {}
    for path in spec_files:
        spec = json.loads(path.read_text(encoding="utf-8"))
        if f"{spec['name']}.json" != path.name:
            fail(spec, f"the file must be named {spec['name']}.json")
        # python-escpos prints a line about the renderer it picked; keep the output readable.
        with contextlib.redirect_stdout(io.StringIO()):
            built = build(spec)
        stream, expected = built["bytes"], built["expected"]

        expected_json = json.dumps(expected, indent=2, ensure_ascii=False) + "\n"
        (OUT / f"{spec['name']}.bin").write_bytes(stream)
        (OUT / f"{spec['name']}.expected.json").write_text(expected_json, encoding="utf-8")
        hashes[spec["name"]] = {"bin": sha256(stream), "expected": sha256(expected_json)}

        bitmap_image = None
        if "bitmap" in built:
            (OUT / f"{spec['name']}.bitmap").write_bytes(built["bitmap"])
            hashes[spec["name"]]["bitmap"] = sha256(built["bitmap"])
            bitmap_image = bits_to_image(built["bitmap"], expected["image"]["width"], expected["image"]["height"])
            bitmap_image.save(PREVIEWS / "png" / f"{spec['name']}.bitmap.png")
        else:
            (PREVIEWS / "hex" / f"{spec['name']}.hex.txt").write_text(hex_dump(stream), encoding="utf-8")

        text = built.get("text")
        preview = render_preview(expected, bitmap_image.convert("L") if bitmap_image else None, text.split("\n") if text else None)
        if preview is not None:
            preview.save(PREVIEWS / "png" / f"{spec['name']}.png")
        if text:
            (PREVIEWS / "text" / f"{spec['name']}.txt").write_text(text + "\n", encoding="utf-8")
        print(f"{spec['name']}: {len(stream)} bytes")

    if UPDATE_LOCK:
        LOCK.write_text(json.dumps(hashes, indent=2) + "\n", encoding="utf-8")
        print(f"\nLock updated: {len(spec_files)} fixtures.")
        return 0

    try:
        lock = json.loads(LOCK.read_text(encoding="utf-8"))
    except FileNotFoundError:
        print("\nfixtures.lock.json is missing. Run: python generate.py --update-lock", file=sys.stderr)
        return 1
    problems = []
    for name, files in hashes.items():
        if name not in lock:
            problems.append(f"{name} is new and not in the lock")
            continue
        for kind, digest in files.items():
            if lock[name].get(kind) != digest:
                problems.append(f"{name}.{kind} changed")
    problems += [f"{name} is in the lock but has no spec" for name in lock if name not in hashes]
    if problems:
        print("\nFixture drift. A spec, the library or a font changed what is generated:", file=sys.stderr)
        for problem in problems:
            print(f"  - {problem}", file=sys.stderr)
        print("Look at the previews in fixtures/out/previews/, then run: python generate.py --update-lock", file=sys.stderr)
        return 1
    print(f"\nAll {len(spec_files)} fixtures match the lock.")
    return 0


if __name__ == "__main__":
    try:
        sys.exit(main())
    except SpecError as error:
        print(error, file=sys.stderr)
        sys.exit(1)
