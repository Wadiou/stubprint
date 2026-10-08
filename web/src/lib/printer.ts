import type { PrinterInfo } from "@/api/generated/types.gen";

export type Tone = "ok" | "warn" | "bad";

/** How a printer is doing, in a word and a colour, from what it says about itself. */
export function printerState(status: PrinterInfo["status"]): { label: string; tone: Tone } {
  if (!status.online) return { label: "Offline", tone: "bad" };
  if (status.cover_open) return { label: "Cover open", tone: "bad" };
  if (status.fault) return { label: `Fault: ${status.fault}`, tone: "bad" };
  if (status.paper === "end") return { label: "Paper end", tone: "bad" };
  if (status.paper === "near_end") return { label: "Paper near end", tone: "warn" };
  return { label: "Online", tone: "ok" };
}

/** The name a new printer gets: `printer`, or with `_2`, `_3` and so on after it, the first that is not taken. */
export function freeName(taken: string[], wanted = "printer") {
  if (!taken.includes(wanted)) return wanted;
  for (let number = 2; ; number++) {
    const name = `${wanted}_${number}`;
    if (!taken.includes(name)) return name;
  }
}

/** The port of a printer, such as `:9100`. */
export const port = (printer: Pick<PrinterInfo, "listen">) => printer.listen.slice(printer.listen.lastIndexOf(":"));
