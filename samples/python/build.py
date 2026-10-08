"""Prints the receipts in receipts/*.json with python-escpos and node/build.mjs, and writes out/.

    python python/build.py                  write out/
    python python/build.py --update-lock    the same, then accept the bytes as the new lock
    python python/build.py --no-previews    skip the pictures, which need cargo

out/python and out/node hold the bytes and a picture of each variant. out/samples.json is what
the server embeds. samples.lock.json holds a hash of every variant.
"""
import base64
import hashlib
import importlib.metadata
import json
import shutil
import subprocess
import sys
from pathlib import Path

from escpos import constants
from escpos.printer import Dummy

ROOT = Path(__file__).resolve().parent.parent
RECEIPTS = ROOT / "receipts"
ASSETS = ROOT / "assets"
OUT = ROOT / "out"
LOCK = ROOT / "samples.lock.json"
NODE = ROOT / "node" / "build.mjs"
WORKSPACE = ROOT.parent
PROFILES = json.loads((ROOT.parent / "profiles" / "profiles.json").read_text())["profiles"]

PYTHON_ESCPOS = "python-escpos"
NODE_THERMAL_PRINTER = "node-thermal-printer"
LANGUAGES = {PYTHON_ESCPOS: "python", NODE_THERMAL_PRINTER: "node"}

SYMBOLOGIES = {"upca": "UPC-A", "upce": "UPC-E", "ean13": "EAN13", "ean8": "EAN8", "code39": "CODE39", "itf": "ITF", "codabar": "NW7", "code93": "CODE93", "code128": "CODE128"}
TEXT_POSITIONS = {"none": "OFF", "above": "ABOVE", "below": "BELOW", "both": "BOTH"}
QR_LEVELS = {"l": constants.QR_ECLEVEL_L, "m": constants.QR_ECLEVEL_M, "q": constants.QR_ECLEVEL_Q, "h": constants.QR_ECLEVEL_H}


def row_text(left, right, columns):
    return left + " " * max(1, columns - len(left) - len(right)) + right


def columns_of(spec):
    """The characters on a line of the printer the receipt is written for, in its first font."""
    return PROFILES[spec["profile"]]["fonts"]["0"]["columns"]


def specs():
    found = [json.loads(path.read_text()) for path in sorted(RECEIPTS.glob("*.json"))]
    return sorted(found, key=lambda spec: spec["order"])


def expected(spec):
    """What the decoder must find on the paper."""
    columns = columns_of(spec)
    answer = {"texts": [], "barcodes": [], "qrs": [], "pdf417": [], "images": 0, "cuts": 0}
    for block in spec["blocks"]:
        if "text" in block and block["text"]:
            answer["texts"].append(block["text"])
        elif "row" in block:
            answer["texts"].append(row_text(*block["row"], columns))
        elif "rule" in block:
            answer["texts"].append(block["rule"] * columns)
        elif "barcode" in block:
            answer["barcodes"].append({"symbology": block["barcode"]["type"], "data": block["barcode"]["data"]})
        elif "qr" in block:
            answer["qrs"].append({"data": block["qr"]["data"]})
        elif "pdf417" in block:
            answer["pdf417"].append({"data": block["pdf417"]["data"]})
        elif "image" in block:
            answer["images"] += 1
        elif "cut" in block:
            answer["cuts"] += 1
    return answer


def style(printer, block):
    width, height = block.get("size", [1, 1])
    printer.set(
        align=block.get("align", "left"),
        font=block.get("font", "a"),
        bold=block.get("bold", False),
        underline=block.get("underline", 0),
        invert=block.get("invert", False),
        flip=block.get("flip", False),
        normal_textsize=(width, height) == (1, 1),
        custom_size=(width, height) != (1, 1),
        width=width,
        height=height,
    )


def with_python_escpos(spec):
    printer = Dummy()
    for block in spec["blocks"]:
        if "text" in block:
            style(printer, block)
            printer.textln(block["text"])
        elif "row" in block:
            style(printer, block)
            printer.textln(row_text(*block["row"], columns_of(spec)))
        elif "rule" in block:
            style(printer, {})
            printer.textln(block["rule"] * columns_of(spec))
        elif "feed" in block:
            printer.ln(block["feed"])
        elif "barcode" in block:
            code = block["barcode"]
            style(printer, {"align": block.get("align")})
            # Code 128 data starts with its character set
            data = "{B" + code["data"] if code["type"] == "code128" else code["data"]
            printer.barcode(data, SYMBOLOGIES[code["type"]], height=code["height"], width=code["width"], pos=TEXT_POSITIONS[code["text"]], align_ct=False)
        elif "qr" in block:
            style(printer, {"align": block.get("align")})
            printer.qr(block["qr"]["data"], ec=QR_LEVELS[block["qr"]["level"]], size=block["qr"]["size"], native=True, center=False)
        elif "image" in block:
            style(printer, {"align": block.get("align")})
            printer.image(str(ASSETS / block["image"]), center=False)
        elif "cut" in block:
            printer.cut()
        else:
            raise SystemExit(f"{spec['id']}: python-escpos cannot print {json.dumps(block)}")
    return printer.output


def with_node_thermal_printer():
    """Runs node/build.mjs, which prints the receipts that ask for node-thermal-printer."""
    if not (ROOT / "node" / "node_modules").is_dir():
        raise SystemExit("run `npm ci` in samples/node first")
    folder = OUT / "node"
    folder.mkdir(parents=True, exist_ok=True)
    done = subprocess.run(["node", str(NODE), str(RECEIPTS), str(ASSETS), str(folder)], check=True, capture_output=True, text=True)
    return {
        (made["id"], made["dialect"]): (made["version"], (folder / made["file"]).read_bytes())
        for made in json.loads(done.stdout)
    }


def preview(folder, name, dialect, profile):
    """Draws a variant with Stubprint's renderer, on the paper of the printer it was written for."""
    subprocess.run(
        ["cargo", "run", "-q", "-p", "stubprint-render", "--example", "render", "--", str(folder / f"{name}.bin"), str(folder / f"{name}.png"), dialect, profile],
        cwd=WORKSPACE, check=True, capture_output=True,
    )


def build(previews):
    node = with_node_thermal_printer()
    python_version = importlib.metadata.version(PYTHON_ESCPOS)
    samples, hashes = [], {}
    for spec in specs():
        variants = []
        for variant in spec["variants"]:
            library, dialect = variant["library"], variant["dialect"]
            if library == PYTHON_ESCPOS:
                version, data = python_version, bytes(with_python_escpos(spec))
            elif library == NODE_THERMAL_PRINTER:
                version, data = node[(spec["id"], dialect)]
            else:
                raise SystemExit(f"{spec['id']}: no library called {library}")
            hashes[f"{spec['id']}/{library}/{dialect}"] = {"version": version, "sha256": hashlib.sha256(data).hexdigest()}
            folder = OUT / LANGUAGES[library]
            folder.mkdir(parents=True, exist_ok=True)
            name = f"{spec['id']}.{library}.{dialect}"
            (folder / f"{name}.bin").write_bytes(data)
            if previews:
                preview(folder, name, dialect, spec["profile"])
            variants.append({"library": library, "version": version, "language": LANGUAGES[library], "dialect": dialect, "bytes": base64.b64encode(data).decode()})
        samples.append({
            "id": spec["id"],
            "title": spec["title"],
            "about": spec["about"],
            "profile": spec["profile"],
            "tags": spec["tags"],
            "expect": expected(spec),
            "receipt": spec,
            "variants": variants,
        })
    return samples, hashes


def check_lock(hashes):
    """Fails when a library wrote other bytes than the lock says."""
    locked = json.loads(LOCK.read_text()) if LOCK.exists() else {}
    problems = [f"{key}: {'new' if key not in locked else 'bytes changed'}" for key, value in hashes.items() if locked.get(key) != value]
    problems += [f"{key}: no longer built" for key in locked if key not in hashes]
    if problems:
        raise SystemExit("samples.lock.json does not match what the libraries wrote:\n  " + "\n  ".join(problems) + "\nIf the change is right, run python/build.py --update-lock")


if __name__ == "__main__":
    samples, hashes = build("--no-previews" not in sys.argv and shutil.which("cargo") is not None)
    if "--update-lock" in sys.argv:
        LOCK.write_text(json.dumps(hashes, indent=2, sort_keys=True) + "\n")
    else:
        check_lock(hashes)
    OUT.mkdir(exist_ok=True)
    (OUT / "samples.json").write_text(json.dumps(samples, indent=2, ensure_ascii=False) + "\n")
    print(f"wrote out/samples.json: {len(samples)} samples, {sum(len(s['variants']) for s in samples)} variants")
