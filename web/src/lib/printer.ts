import { z } from "zod";
import type { NetworkAddress, PrinterInfo } from "@/api/generated/types.gen";

/** A port as typed: digits only, from 1 to 65535. */
const typedPort = z
  .string()
  .regex(/^\d+$/)
  .transform(Number)
  .pipe(z.number().int().min(1).max(65535));

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

/** The port of a printer, such as `9100`. */
export const port = (printer: Pick<PrinterInfo, "listen">) => printer.listen.slice(printer.listen.lastIndexOf(":") + 1);

/** The address a printer listens on. A port that is not given is 0, and the system picks one. */
export function listenAddress(host: string, portText: string) {
  return `${host}:${portText || "0"}`;
}

/** The addresses that a program types to reach a printer. One that listens on every address of the computer can be reached on each of them. */
export function connectTo(listen: string, addresses: NetworkAddress[]) {
  const host = listen.slice(0, listen.lastIndexOf(":"));
  if (host !== "0.0.0.0" && host !== "[::]") return [listen];
  const number = port({ listen });
  return ["127.0.0.1", ...addresses.map((entry) => entry.address)].map((address) => `${address}:${number}`);
}

/** What the form for the address of a printer holds: the address, whether the system picks the port, and the port otherwise. */
export interface ListenChoice {
  host: string;
  free: boolean;
  port: string;
}

export const DEFAULT_LISTEN: ListenChoice = { host: "127.0.0.1", free: true, port: "9100" };

/** The address a choice asks for, and whether it is complete: an address, and a port from 1 to 65535 unless the system picks it. */
export function resolveListen({ host, free, port: portText }: ListenChoice) {
  const valid = host.length > 0 && (free || typedPort.safeParse(portText).success);
  return { valid, listen: listenAddress(host, free ? "" : portText) };
}
