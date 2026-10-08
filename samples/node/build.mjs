// Builds the Node variants of the samples. It reads the receipts in samples/receipts and prints
// each with node-thermal-printer, which only collects the bytes it would send to a printer.
//
//     node build.mjs <receipts folder> <assets folder> <out folder>
//
// It writes one .bin for each variant and says what it wrote as JSON on its output.
import { createRequire } from "node:module";
import { mkdirSync, readdirSync, readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";

const require = createRequire(import.meta.url);
const { printer: ThermalPrinter, types: PrinterTypes } = require("node-thermal-printer");
const { version } = require("node-thermal-printer/package.json");

const LIBRARY = "node-thermal-printer";
const TYPES = { escpos: PrinterTypes.EPSON, starline: PrinterTypes.STAR };
// The numbers each family of commands gives the symbologies.
const BARCODES = {
  escpos: { upca: 65, upce: 66, ean13: 67, ean8: 68, code39: 69, itf: 70, codabar: 71, code93: 72, code128: 73 },
  starline: { upce: 0, upca: 1, ean8: 2, ean13: 3, code39: 4, itf: 5, code128: 6, code93: 7, codabar: 8 },
};
const TEXT_POSITIONS = { none: 0, above: 1, below: 2, both: 3 };

const [receipts, assets, out] = process.argv.slice(2);
mkdirSync(out, { recursive: true });

function align(printer, name) {
  ({ left: () => printer.alignLeft(), center: () => printer.alignCenter(), right: () => printer.alignRight() })[name ?? "left"]();
}

// Star's commands in this library know four sizes: normal, double width, double height and both.
function size(printer, dialect, width, height) {
  if (dialect === "escpos") {
    // The library is documented as setTextSize(height, width), but it writes its first argument
    // as the width, so a double height line needs the arguments the other way round.
    return width === 1 && height === 1 ? printer.setTextNormal() : printer.setTextSize(width - 1, height - 1);
  }
  if (width === 1 && height === 1) return printer.setTextNormal();
  if (height === 1) return printer.setTextDoubleWidth();
  if (width === 1) return printer.setTextDoubleHeight();
  return printer.setTextQuadArea();
}

function style(printer, dialect, block) {
  const [width, height] = block.size ?? [1, 1];
  // The size comes first: "normal text" resets the font, bold and underline too.
  size(printer, dialect, width, height);
  block.font === "b" ? printer.setTypeFontB() : printer.setTypeFontA();
  printer.bold(Boolean(block.bold));
  block.underline === 2 ? printer.underlineThick(true) : printer.underline(block.underline === 1);
  printer.invert(Boolean(block.invert));
  printer.upsideDown(Boolean(block.flip));
  align(printer, block.align);
}

async function build(spec, dialect) {
  const printer = new ThermalPrinter({ type: TYPES[dialect], width: spec.columns });
  for (const block of spec.blocks) {
    if ("text" in block) {
      style(printer, dialect, block);
      printer.println(block.text);
    } else if ("row" in block) {
      style(printer, dialect, block);
      printer.leftRight(block.row[0], block.row[1]);
    } else if ("rule" in block) {
      style(printer, dialect, {});
      printer.drawLine(block.rule);
    } else if ("feed" in block) {
      for (let i = 0; i < block.feed; i++) printer.newLine();
    } else if ("barcode" in block) {
      const code = block.barcode;
      style(printer, dialect, { align: block.align });
      const type = BARCODES[dialect][code.type];
      if (dialect === "escpos") {
        printer.printBarcode(code.data, type, { hriPos: TEXT_POSITIONS[code.text], width: code.width, height: code.height });
      } else {
        printer.printBarcode(code.data, type, { characters: code.text === "none" ? 1 : 2, mode: Math.min(Math.max(code.width - 1, 1), 3), height: code.height });
      }
    } else if ("qr" in block) {
      style(printer, dialect, { align: block.align });
      const qr = block.qr;
      printer.printQR(qr.data, dialect === "escpos"
        ? { model: 2, cellSize: qr.size, correction: qr.level.toUpperCase() }
        : { model: 2, cellSize: qr.size, correctionLevel: qr.level.toUpperCase() });
    } else if ("pdf417" in block) {
      style(printer, dialect, { align: block.align });
      printer.pdf417(block.pdf417.data);
    } else if ("image" in block) {
      style(printer, dialect, { align: block.align });
      await printer.printImage(join(assets, block.image));
    } else if ("cut" in block) {
      printer.cut();
    } else {
      throw new Error(`${spec.id}: unknown block ${JSON.stringify(block)}`);
    }
  }
  return printer.getBuffer();
}

const done = [];
for (const name of readdirSync(receipts).filter((n) => n.endsWith(".json")).sort()) {
  const spec = JSON.parse(readFileSync(join(receipts, name), "utf8"));
  for (const variant of spec.variants.filter((v) => v.library === LIBRARY)) {
    const file = `${spec.id}.${LIBRARY}.${variant.dialect}.bin`;
    writeFileSync(join(out, file), await build(spec, variant.dialect));
    done.push({ id: spec.id, library: LIBRARY, version, dialect: variant.dialect, file });
  }
}
console.log(JSON.stringify(done, null, 2));
