import { expect, test } from "vitest";
import { columnsText, featureLabels, paperText } from "./profile";

test("the paper is told as the model says it and nothing is made up", () => {
  expect(paperText({ width_mm: 80, width_dots: 576, dpi: 203 })).toBe("80 mm, 576 dots at 203 dpi");
  expect(paperText({ width_mm: null, width_dots: 512, dpi: null })).toBe("512 dots");
  expect(paperText({})).toBe("");
});

test("the columns are named by the font and a font the model lacks is skipped", () => {
  expect(columnsText([48, 64])).toBe("48 in font A, 64 in font B");
  expect(columnsText([42, 0, 30])).toBe("42 in font A, 30 in font C");
  expect(columnsText([])).toBe("");
});

test("features are read as words, once each, in a fixed order", () => {
  expect(featureLabels(["paperFullCut", "barcodeB", "barcodeA", "qrCode", "highDensity"])).toEqual([
    "QR code",
    "Barcodes",
    "Full cut",
  ]);
});
