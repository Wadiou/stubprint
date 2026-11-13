import { z } from "zod";
import type { NetworkAddress, PrinterInfo } from "@/api/generated/types.gen";

/** A port as typed: digits only, from 1 to 65535. */
const typedPort = z
  .string()
  .regex(/^\d+$/)
  .transform(Number)
  .pipe(z.number().int().min(1).max(65535));

/** How many bytes a printer holds while it cannot print, when it holds any, if nothing else is chosen. */
export const DEFAULT_PRINTER_BUFFER = 4096;

/** How many bytes the network keeps for what a program sends, when it is limited, if nothing else is chosen. */
export const DEFAULT_NETWORK_BUFFER = 8192;

/** The most that can be asked, so a typing slip does not ask for gigabytes of memory. */
export const MAX_BYTES = 1_048_576;

/** A whole number as typed: digits only, from 1 to `max`. Nothing when it is not one. */
export const wholeNumber = (max: number) => (text: string) => {
  const number = z
    .string()
    .regex(/^\d+$/)
    .transform(Number)
    .pipe(z.number().int().min(1).max(max))
    .safeParse(text.trim());
  return number.success ? number.data : undefined;
};

/** A size as typed, in bytes, from 1 to the most. */
export const byteSize = wholeNumber(MAX_BYTES);

/** How fast a printer prints when it is turned on and no speed is chosen, in millimetres a second: a common receipt printer. */
export const DEFAULT_SPEED = 150;

/** The fastest that can be asked for, in millimetres a second. */
export const MAX_SPEED = 1000;

/** A speed as typed, in millimetres a second, from 1 to the most. */
export const printSpeed = wholeNumber(MAX_SPEED);

/** How many programs a printer takes at once when a limit is turned on and none is chosen: one, as a classic receipt printer does. */
export const DEFAULT_CONNECTION_LIMIT = 1;

/** The most programs a printer takes at once, whatever it is set to. */
export const MAX_CONNECTION_LIMIT = 64;

/** A number of connections as typed, from 1 to the most. */
export const connectionLimit = wholeNumber(MAX_CONNECTION_LIMIT);

/** How long a connection can send nothing, when a time is turned on and none is chosen, in seconds. */
export const DEFAULT_IDLE_TIMEOUT = 30;

/** The longest idle time that can be asked for, in seconds: a day. */
export const MAX_IDLE_TIMEOUT = 86_400;

/** An idle time as typed, in seconds, from 1 to the most. */
export const idleTimeout = wholeNumber(MAX_IDLE_TIMEOUT);

/** How long a roll is when one is put in without a length chosen, in millimetres: 80 m, a common roll. */
export const DEFAULT_ROLL = 80_000;

/** The longest roll that can be asked for, in millimetres: a kilometre. */
export const MAX_ROLL = 1_000_000;

/** A length of paper as typed, in metres with up to three decimals: millimetres. Nothing when it is not one. */
export function rollLength(text: string) {
  const length = z
    .string()
    .regex(/^\d+([.,]\d{1,3})?$/)
    .transform((typed) => Math.round(Number(typed.replace(",", ".")) * 1000))
    .pipe(z.number().int().min(1).max(MAX_ROLL))
    .safeParse(text.trim());
  return length.success ? length.data : undefined;
}

/** A length in millimetres as metres to type or read: `42.3`. */
export const metresText = (millimetres: number) => (millimetres / 1000).toLocaleString("en-US", { maximumFractionDigits: 3, useGrouping: false });

/** How much of a roll is left, as a share from 0 to 1. A printer with no roll to run out has none. */
export const rollShare = (left: number, roll: number) => (roll > 0 ? Math.min(1, Math.max(0, left / roll)) : 0);

/** How much paper is left on the roll: `42.3 m of 80 m`. */
export const leftText = (left: number, roll: number) => `${metresText(left)} m of ${metresText(roll)} m`;

/** How full the buffer of a printer is: `Holding 312 / 4,096 bytes`. */
export const heldText = (held: number, capacity: number) =>
  `Holding ${held.toLocaleString("en-US")} / ${capacity.toLocaleString("en-US")} bytes`;

/** What a toast says about a printer that cannot print and holds what it was sent: why, and how much waits. */
export const stuckText = (status: PrinterInfo["status"], held: number) =>
  `${printerState(status).label} · ${held.toLocaleString("en-US")} bytes waiting`;

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
