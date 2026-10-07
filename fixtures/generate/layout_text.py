"""The receipt as lines of text for the printer's own font, where each character is one column."""
from data import receipt_rows


def wrap(text, columns):
    """Breaks text at spaces so no line is longer than `columns`; a word that is too long is cut."""
    lines = []
    current = ""
    for word in text.split(" "):
        while len(word) > columns:
            if current:
                lines.append(current)
                current = ""
            lines.append(word[:columns])
            word = word[columns:]
        following = f"{current} {word}" if current else word
        if len(following) > columns:
            lines.append(current)
            current = word
        else:
            current = following
    if current:
        lines.append(current)
    return lines


def center(text, columns):
    return " " * ((columns - len(text)) // 2) + text


def justify(start, end, columns):
    return start + " " * max(1, columns - len(start) - len(end)) + end


def text_lines(receipt, columns):
    """Right to left receipts put the label on the right and the amount on the left, and the
    Arabic is left in reading order, which is how a printer without shaping prints it.
    Returns a list of (text, bold)."""
    rtl = receipt["lang"] == "ar"
    lines = []

    def add(text, bold=False):
        lines.append((text, bold))

    for row in receipt_rows(receipt):
        kind = row["kind"]
        if kind == "title":
            for line in wrap(row["text"], columns):
                add(center(line, columns), True)
        elif kind == "center":
            for line in wrap(row["text"], columns):
                add(center(line, columns))
        elif kind == "rule":
            add(("=" if row["style"] == "double" else "-") * columns)
        elif kind == "blank":
            add("")
        elif kind == "pair":
            first, *rest = wrap(row["label"], columns - len(row["value"]) - 1)
            add(justify(row["value"], first, columns) if rtl else justify(first, row["value"], columns), row.get("bold", False))
            for line in rest:
                add(line.rjust(columns) if rtl else line)
        elif kind == "item":
            for line in wrap(row["name"], columns):
                add(line.rjust(columns) if rtl else line)
            add(justify(row["total"], row["detail"], columns) if rtl else justify(f"  {row['detail']}", row["total"], columns))
    return lines
