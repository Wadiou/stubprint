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
    ("thai", False): "NotoSansThai_400Regular.ttf",
    ("thai", True): "NotoSansThai_400Regular.ttf",
}
# The fonts tried in turn for a character that the line's own font has no glyph for.
FALLBACK = ["latin", "thai", "cjk"]
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


def word_runs(word):
    """A word as the stretches one font draws, each with the script of its font. The Arabic font
    draws the Arabic letters and the characters it has that are also in Latin, and another font
    draws a character the Arabic font has no glyph for."""
    if not is_arabic(word):
        return [(word, "latin")]
    arabic = covered("arabic")
    out = []
    for c in word:
        script = "arabic" if is_arabic(c) or (ord(c) in arabic and script_of(c) == "latin") else script_of(c)
        if out and out[-1][1] == script:
            out[-1][0] += c
        else:
            out.append([c, script])
    return [(chunk, script) for chunk, script in out]


def _word_runs(word, bold, size):
    """The runs of a word with the font and the options Pillow needs to draw each."""
    return [
        (chunk, font(script, bold, size), {"direction": "rtl", "language": "ar"} if script == "arabic" else {"direction": "ltr"})
        for chunk, script in word_runs(word)
    ]


@lru_cache(maxsize=None)
def covered(script):
    """The characters a font has a glyph for."""
    from fontTools.ttLib import TTFont

    return frozenset(TTFont(str(FONTS / FILES[(script, False)]), lazy=True).getBestCmap())


def invisible(character):
    """A character that prints nothing: a control code, such as the ones a code page leaves empty."""
    return ord(character) < 0x20 or 0x7F <= ord(character) <= 0x9F


def visible(text):
    """The text as a printer prints it: positions that hold no character print nothing, and the
    soft hyphen, which a printer has a glyph for, prints as a hyphen."""
    return "".join("-" if c == "\u00ad" else c for c in text if not invisible(c) or c == "\t")


def script_of(character):
    """Which font draws a character: Japanese, the private use area where the decoder puts the
    characters a program drew, otherwise the first font that has a glyph for it, which is Latin
    when none has."""
    code = ord(character)
    if 0x3000 <= code <= 0x30FF or 0x4E00 <= code <= 0x9FFF or 0xFF00 <= code <= 0xFFEF:
        return "cjk"
    if 0xE000 <= code <= 0xE1FF:
        return "user"
    return next((script for script in FALLBACK if code in covered(script)), "latin")


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
    text = visible(text)
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
    placed = [[(chunk, f, options, f.getlength(chunk, **options)) for chunk, f, options in _word_runs(word, bold, size)] for word in words]
    widths = [sum(run[3] for run in runs_) for runs_ in placed]
    total = sum(widths) + space * (len(words) - 1)
    right = {"right": x, "left": x + total, "center": x + total / 2}[align]
    for runs_, width in zip(placed, widths):
        edge = right
        for chunk, f, options, run_width in runs_:
            draw.text((edge, y), chunk, font=f, fill=0, anchor="rs", **options)
            edge -= run_width
        right -= width + space
