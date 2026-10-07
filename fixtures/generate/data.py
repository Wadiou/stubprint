"""One small, fixed receipt in three languages. It is only here to have something to print:
a header, two items, a total, and a code. The fixtures are about how the printer commands are
decoded, not about the shop."""

TEXT = {
    "en": {
        "store": "Green Market",
        "address": "12 Main Street, Springfield",
        "items": ["Whole milk 1 L", "White sugar 1 kg"],
        "labels": {"order": "Order", "total": "TOTAL", "thanks": "Thank you", "currency": "USD"},
    },
    "fr": {
        "store": "Marché Vert",
        "address": "12 rue Principale, Springfield",
        "items": ["Lait entier 1 L", "Café moulu 250 g"],
        "labels": {"order": "Commande", "total": "TOTAL", "thanks": "Merci de votre visite", "currency": "EUR"},
    },
    "ar": {
        "store": "السوق الأخضر",
        "address": "12 الشارع الرئيسي، المدينة",
        "items": ["حليب كامل الدسم 1 لتر", "سكر أبيض 1 كغ"],
        "labels": {"order": "الطلب", "total": "المجموع", "thanks": "شكرا لزيارتكم", "currency": "دج"},
    },
}


def money(cents, lang):
    """46050 cents is 460.50 in English and Arabic and 460,50 in French."""
    whole = str(cents // 100)
    fraction = str(cents % 100).rjust(2, "0")
    return f"{whole}{',' if lang == 'fr' else '.'}{fraction}"


def make_receipt(lang):
    text = TEXT.get(lang)
    if text is None:
        raise ValueError(f'no receipt text for the language "{lang}"')
    lines = [
        {"name": text["items"][0], "quantity": 2, "unit": 14000},
        {"name": text["items"][1], "quantity": 1, "unit": 18050},
    ]
    return {
        "lang": lang,
        "code": "8F2A",
        "store": text["store"],
        "address": text["address"],
        "labels": text["labels"],
        "date": "2026-10-07 14:32",
        "lines": lines,
        "total": sum(line["quantity"] * line["unit"] for line in lines),
    }


def receipt_rows(receipt):
    """The receipt as rows, in reading order. Both ways of printing it, with the printer's own
    fonts and as a picture, are drawn from these same rows."""
    labels, lang = receipt["labels"], receipt["lang"]
    rows = [
        {"kind": "title", "text": receipt["store"]},
        {"kind": "center", "text": receipt["address"]},
        {"kind": "rule", "style": "double"},
        {"kind": "pair", "label": f"{labels['order']} {receipt['code']}", "value": receipt["date"]},
        {"kind": "rule", "style": "single"},
    ]
    for line in receipt["lines"]:
        rows.append(
            {
                "kind": "item",
                "name": line["name"],
                "detail": f"{line['quantity']} x {money(line['unit'], lang)}",
                "total": money(line["quantity"] * line["unit"], lang),
            }
        )
    rows += [
        {"kind": "rule", "style": "single"},
        {
            "kind": "pair",
            "label": labels["total"],
            "value": f"{money(receipt['total'], lang)} {labels['currency']}",
            "bold": True,
        },
        {"kind": "blank"},
        {"kind": "center", "text": labels["thanks"]},
    ]
    return rows
