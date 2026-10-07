"""Finds characters in the preview text that the font which draws them has no glyph for, which
show as empty boxes in the pictures. It uses the same font choice as shaping.py and reads the
font tables, so nobody has to look at the pictures.

    python check_glyphs.py          # after generate.py; exits 1 when a character has no glyph
"""
import json
import sys
from pathlib import Path

from preview import _drawn_characters
from shaping import covered, is_arabic, script_of, visible, word_runs

OUT = Path(__file__).resolve().parent.parent / "out"


def missing(line):
    """The characters of a line that have no glyph in the font that draws them, with the font."""
    line = visible(line)
    if is_arabic(line):
        pieces = [run for word in line.split(" ") for run in word_runs(word)]
    else:
        pieces = [(c, script_of(c)) for c in line]
    return [
        (c, script)
        for chunk, script in pieces
        for c in chunk
        if script != "user" and ord(c) > 0x20 and ord(c) not in covered(script)
    ]


def lines_of(name):
    """The lines a preview draws: the laid out text when there is one, else the expected texts.
    Also the characters a program drew, which are drawn from their dots."""
    text = OUT / "previews" / "text" / f"{name}.txt"
    expected = json.loads((OUT / "escpos" / f"{name}.expected.json").read_text(encoding="utf-8"))
    if text.exists():
        return text.read_text(encoding="utf-8").split("\n"), expected
    return expected.get("texts", []), expected


def main():
    problems = 0
    checked = 0
    for path in sorted((OUT / "escpos").glob("*.expected.json")):
        name = path.name[: -len(".expected.json")]
        lines, expected = lines_of(name)
        drawn = _drawn_characters(expected)
        for number, line in enumerate(lines, 1):
            checked += 1
            found = list(missing(line))
            found += [(c, "drawn characters") for c in line if script_of(c) == "user" and c not in drawn]
            found += [(c, "decoder (a byte the code page has no character for)") for c in line if c == "\ufffd"]
            for c, script in found:
                problems += 1
                print(f"{name}:{number}: U+{ord(c):04X} {c!r} has no glyph in the {script}: {line!r}")
    print(f"{checked} lines checked, {problems} characters without a glyph")
    return 1 if problems else 0


if __name__ == "__main__":
    sys.exit(main())
