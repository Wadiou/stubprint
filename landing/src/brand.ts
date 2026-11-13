/**
 * What the page and the pictures drawn for it (the icons and the card that links show) both say, so
 * `pnpm brand` and the page cannot drift apart. Only erasable TypeScript goes here: the script of
 * the pictures is plain Node, which reads this file as it is.
 */

export const NAME = "Stubprint";
export const DOMAIN = "stubprint.dev";
export const REPO = "https://github.com/Wadiou/stubprint";

export const HEADLINE = "A thermal printer simulator for development and testing";
/** `**word**` marks what the page highlights when it shows this, as markdown bold does in the README. */
export const TAGLINE =
  "A thermal printer simulator for development and testing, built in **Rust**. It reads what a program would send to a real printer and shows exactly what would have come out, through a **web app** or a **CLI**, on any of **50 real printer models**.";

/** The colours of the page, which are the ones of the web page of the program. */
export const COLORS = {
  background: "#09090B",
  panel: "#18181B",
  border: "#27272A",
  muted: "#A1A1AA",
  foreground: "#FAFAFA",
  /** The paper of a receipt. */
  paper: "#FFFFFF",
} as const;
