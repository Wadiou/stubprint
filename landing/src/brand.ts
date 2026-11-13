/**
 * What the page and the pictures drawn for it (the icons and the card that links show) both say, so
 * `pnpm brand` and the page cannot drift apart. Only erasable TypeScript goes here: the script of
 * the pictures is plain Node, which reads this file as it is.
 */

export const NAME = "Stubprint";
export const DOMAIN = "stubprint.dev";
export const REPO = "https://github.com/Wadiou/stubprint";

export const HEADLINE = "A thermal printer simulator for development and testing";
export const TAGLINE =
  "Print to it like a real network thermal printer, in ESC/POS or Star, and see exactly what would have come out: no printer, no paper, no trip to the shop.";

/** The colours of the page, which are the ones of the web page of the program. */
export const COLORS = {
  background: "#09090B",
  panel: "#18181B",
  border: "#27272A",
  muted: "#A1A1AA",
  foreground: "#FAFAFA",
  /** The paper of a receipt, and its ink. */
  paper: "#FAF8F2",
  ink: "#1B1B1B",
} as const;

/** The receipt that the page and the card show: what the bytes of the example print. */
export const RECEIPT = [
  { text: "GREEN MARKET", style: "title" },
  { text: "12 Main Street", style: "centre" },
  { text: "------------------------------" },
  { text: "2 x Coffee                7.00" },
  { text: "1 x Croissant             3.40" },
  { text: "------------------------------" },
  { text: "TOTAL                    10.40", style: "bold" },
] as const;
