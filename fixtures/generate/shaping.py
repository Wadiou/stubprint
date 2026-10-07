"""Draws one line of Latin, Japanese or Arabic text, and the characters a program drew, with Pillow.

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
    # One weight, cut down to the Japanese letters and the 6,355 Kanji of JIS X 0208 level 1 and 2
    # by fonts/make_noto_sans_jp_subset.py, which is also what Chinese text in the fixtures uses.
    ("cjk", False): "NotoSansJP_400Regular_subset.ttf",
    ("cjk", True): "NotoSansJP_400Regular_subset.ttf",
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


def script_of(character):
    """Which font draws a character: Japanese, the private use area where the decoder puts the
    characters a program drew, or Latin for the rest."""
    code = ord(character)
    if 0x3000 <= code <= 0x30FF or 0x4E00 <= code <= 0x9FFF or 0xFF00 <= code <= 0xFFEF:
        return "cjk"
    if 0xE000 <= code <= 0xE1FF:
        return "user"
    return "latin"


def runs(text):
    """Cuts a line into stretches that one font, or the drawn characters, can draw."""
    out = []
    for character in text:
        script = script_of(character)
        if out and out[-1][0] == script:
            out[-1][1] += character
        else:
            out.append([script, character])
    return out


def _run_width(script, chunk, bold, size, drawn):
    if script == "user":
        return sum(_user_width(drawn, c, size) for c in chunk)
    return font(script, bold, size).getlength(chunk)


def _user_width(drawn, character, size):
    glyph = (drawn or {}).get(character)
    return glyph["width"] * size / glyph["height"] if glyph else size / 2


def _draw_user(draw, drawn, character, x, baseline, size):
    """Draws the dots of a character a program drew, as tall as the line."""
    glyph = (drawn or {}).get(character)
    if glyph is None:
        return
    scale = size / glyph["height"]
    top = baseline - size * 0.8
    for column, data in enumerate(glyph["columns"]):
        for row in range(glyph["height"]):
            if data[row // 8] & (0x80 >> (row % 8)):
                left = x + column * scale
                above = top + row * scale
                draw.rectangle((left, above, max(left, left + scale - 1), max(above, above + scale - 1)), fill=0)


def draw_line(draw, text, x, y, align, size, bold=False, rtl=False, drawn=None):
    """Draws `text` with its baseline at y. `align` says which edge of the text is at x. `drawn`
    holds the characters a program drew, by the private use character that stands for each."""
    if not (rtl or is_arabic(text)):
        pieces = runs(text)
        if len(pieces) == 1 and pieces[0][0] != "user":
            # One font draws all of it, and Pillow places it by the anchor.
            draw.text((x, y), text, font=font(pieces[0][0], bold, size), fill=0, anchor=ANCHORS[align])
            return
        total = sum(_run_width(script, chunk, bold, size, drawn) for script, chunk in pieces)
        left = {"left": x, "center": x - total / 2, "right": x - total}[align]
        for script, chunk in pieces:
            if script == "user":
                for character in chunk:
                    _draw_user(draw, drawn, character, left, y, size)
                    left += _user_width(drawn, character, size)
            else:
                draw.text((left, y), chunk, font=font(script, bold, size), fill=0, anchor="ls")
                left += font(script, bold, size).getlength(chunk)
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
