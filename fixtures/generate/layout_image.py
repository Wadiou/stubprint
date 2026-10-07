"""Draws the receipt as a picture, so any language prints."""
import math

from PIL import Image, ImageDraw

from data import receipt_rows
from shaping import draw_line


def wrap(text, columns):
    """Breaks text at spaces so no line is longer than `columns`."""
    lines = []
    current = ""
    for word in text.split(" "):
        following = f"{current} {word}" if current else word
        if len(following) > columns and current:
            lines.append(current)
            current = word
        else:
            current = following
    if current:
        lines.append(current)
    return lines


def draw_receipt(receipt, width):
    """Draws the receipt `width` dots wide, in pure black and white. Returns the picture and the
    packed bits (black is 1) a printer must end up with."""
    rtl = receipt["lang"] == "ar"
    margin = 12
    size = 22 if width == 384 else 26
    line_height = round(size * 1.5)
    max_chars = math.floor((width - margin * 2) / (size * 0.6))
    start = width - margin if rtl else margin
    end = margin if rtl else width - margin
    start_align = "right" if rtl else "left"
    end_align = "left" if rtl else "right"

    y = line_height
    operations = []

    def text(value, x, align, bold=False, scale=1.0):
        operations.append(("text", value, x, y, align, round(size * scale), bold))

    def rule(double):
        for offset in [-size / 2] + ([-size / 2 + 5] if double else []):
            operations.append(("line", y + offset))

    for row in receipt_rows(receipt):
        kind = row["kind"]
        if kind == "title":
            text(row["text"], width / 2, "center", bold=True, scale=1.3)
            y += line_height * 1.2
        elif kind == "center":
            for line in wrap(row["text"], max_chars):
                text(line, width / 2, "center")
                y += line_height
        elif kind == "rule":
            rule(row["style"] == "double")
            y += line_height * 0.7
        elif kind == "blank":
            y += line_height
        elif kind == "pair":
            bold = row.get("bold", False)
            text(row["label"], start, start_align, bold=bold, scale=1.15 if bold else 1)
            text(row["value"], end, end_align, bold=bold, scale=1.15 if bold else 1)
            y += line_height
        elif kind == "item":
            for line in wrap(row["name"], max_chars):
                text(line, start, start_align)
                y += line_height
            text(row["detail"], start, start_align)
            text(row["total"], end, end_align)
            y += line_height

    height = math.ceil((y + line_height / 2) / 8) * 8
    image = Image.new("L", (width, height), 255)
    draw = ImageDraw.Draw(image)
    for operation in operations:
        if operation[0] == "line":
            line_y = round(operation[1])
            draw.line([(margin, line_y), (width - margin, line_y)], fill=0, width=2)
        else:
            _, value, x, baseline, align, font_size, bold = operation
            draw_line(draw, value, x, round(baseline), align, font_size, bold, rtl)

    # Anti-aliasing leaves greys; make every pixel pure black or white before anyone thresholds it.
    image = image.point(lambda value: 0 if value < 128 else 255)
    return image, pack_bits(image)


def pack_bits(image):
    """The rows of a black and white picture as bytes, 8 pixels to a byte, black is 1."""
    width, height = image.size
    pixels = image.tobytes()
    row_bytes = (width + 7) // 8
    bits = bytearray(row_bytes * height)
    for y in range(height):
        for x in range(width):
            if pixels[y * width + x] < 128:
                bits[y * row_bytes + (x >> 3)] |= 0x80 >> (x & 7)
    return bytes(bits)
