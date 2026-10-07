"""Draws one line of Latin or Arabic text with Pillow.

Pillow shapes and orders a single font run, but a receipt line mixes Arabic words with digits and
codes like 8F2A, which need another font. So a right to left line is cut into words, each word is
drawn in the font that has its letters, and the words are placed from the right, which is the
order a right to left paragraph reads in. That is enough for the short fixed lines used here.
"""
from functools import lru_cache
from pathlib import Path

from PIL import ImageFont, features

FONTS = Path(__file__).resolve().parent / "fonts"
FILES = {
    ("latin", False): "NotoSansMono_400Regular.ttf",
    ("latin", True): "NotoSansMono_700Bold.ttf",
    ("arabic", False): "NotoNaskhArabic_400Regular.ttf",
    ("arabic", True): "NotoNaskhArabic_700Bold.ttf",
}
ANCHORS = {"left": "ls", "center": "ms", "right": "rs"}


def require_raqm():
    """Arabic letters only join into words when Pillow has the Raqm layout engine."""
    if not features.check("raqm"):
        raise SystemExit(
            "Pillow has no Raqm text layout, so Arabic would not be shaped.\n"
            "It needs the FriBiDi library: sudo apt install libfribidi0 libharfbuzz0b libraqm0"
        )


@lru_cache(maxsize=None)
def font(script, bold, size):
    return ImageFont.truetype(str(FONTS / FILES[(script, bold)]), size, layout_engine=ImageFont.Layout.RAQM)


def is_arabic(text):
    return any("؀" <= c <= "ۿ" or "ݐ" <= c <= "ݿ" or "ﭐ" <= c <= "﻿" for c in text)


def _word(word, bold, size):
    """The font, and the options Pillow needs, to draw one word."""
    if is_arabic(word):
        return font("arabic", bold, size), {"direction": "rtl", "language": "ar"}
    return font("latin", bold, size), {"direction": "ltr"}


def draw_line(draw, text, x, y, align, size, bold=False, rtl=False):
    """Draws `text` with its baseline at y. `align` says which edge of the text is at x."""
    if not (rtl or is_arabic(text)):
        draw.text((x, y), text, font=font("latin", bold, size), fill=0, anchor=ANCHORS[align])
        return

    words = text.split(" ")
    space = font("latin", bold, size).getlength(" ")
    fonts = [_word(word, bold, size) for word in words]
    widths = [f.getlength(word, **options) for word, (f, options) in zip(words, fonts)]
    total = sum(widths) + space * (len(words) - 1)
    right = {"right": x, "left": x + total, "center": x + total / 2}[align]
    for word, width, (f, options) in zip(words, widths, fonts):
        draw.text((right, y), word, font=f, fill=0, anchor="rs", **options)
        right -= width + space
