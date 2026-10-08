import type { Dialect, ProfileInfo } from "@/api/generated/types.gen";

export const DIALECTS: Record<Dialect, string> = {
  escpos: "ESC/POS",
  starline: "Star Line",
  starprnt: "StarPRNT",
};

// What a person reads for the features of the database, in the order they are shown.
const FEATURES: [string[], string][] = [
  [["qrCode"], "QR code"],
  [["pdf417Code"], "PDF417"],
  [["barcodeA", "barcodeB"], "Barcodes"],
  [["bitImageRaster", "bitImageColumn", "graphics"], "Images"],
  [["paperFullCut"], "Full cut"],
  [["paperPartCut"], "Partial cut"],
  [["pulseStandard"], "Cash drawer"],
  [["pulseBel"], "Buzzer"],
  [["starCommands"], "Star commands"],
];

export function featureLabels(features: string[]) {
  return FEATURES.filter(([names]) => names.some((name) => features.includes(name))).map(([, label]) => label);
}

/** The paper as the model says it: `80 mm, 576 dots at 203 dpi`, with what is not known left out. It is empty when nothing is known. */
export function paperText(profile: Pick<ProfileInfo, "width_mm" | "width_dots" | "dpi">) {
  const parts = [
    profile.width_mm ? `${profile.width_mm} mm` : undefined,
    profile.width_dots ? `${profile.width_dots} dots${profile.dpi ? ` at ${profile.dpi} dpi` : ""}` : undefined,
  ];
  return parts.filter(Boolean).join(", ");
}

/** How many characters fit on a line in each font that the model has: `48 in font A, 64 in font B`. It is empty when nothing is known. */
export function columnsText(columns: number[]) {
  const fonts = columns.flatMap((count, index) =>
    count > 0 ? [`${count} in font ${String.fromCharCode(65 + index)}`] : [],
  );
  return fonts.join(", ");
}
