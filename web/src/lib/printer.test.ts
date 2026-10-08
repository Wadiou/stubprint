import { expect, test } from "vitest";
import { connectTo, DEFAULT_LISTEN, freeName, port, printerState, resolveListen } from "./printer";

const fine = { online: true, cover_open: false, drawer_open: false, feed_button: false, paper: "adequate" } as const;

test("a printer is online until something is wrong, and the worst thing is said first", () => {
  expect(printerState(fine)).toEqual({ label: "Online", tone: "ok" });
  expect(printerState({ ...fine, paper: "near_end" })).toEqual({ label: "Paper near end", tone: "warn" });
  expect(printerState({ ...fine, paper: "end" })).toEqual({ label: "Paper end", tone: "bad" });
  expect(printerState({ ...fine, fault: "cutter" })).toEqual({ label: "Fault: cutter", tone: "bad" });
  expect(printerState({ ...fine, cover_open: true, paper: "end" })).toEqual({ label: "Cover open", tone: "bad" });
  expect(printerState({ ...fine, online: false, cover_open: true })).toEqual({ label: "Offline", tone: "bad" });
});

test("a new printer is called printer, and with a number after it when that is taken", () => {
  expect(freeName([])).toBe("printer");
  expect(freeName(["printer", "front"])).toBe("printer_2");
  expect(freeName(["printer", "printer_2", "printer_4"])).toBe("printer_3");
});

test("the port is what follows the last colon", () => {
  expect(port({ listen: "127.0.0.1:9100" })).toBe("9100");
  expect(port({ listen: "[::1]:9101" })).toBe("9101");
});

const wifi = { name: "wlan0", address: "192.168.1.20" };

test("a program reaches a printer on the address it listens on, and on each address of the computer when it listens on all", () => {
  expect(connectTo("127.0.0.1:9100", [wifi])).toEqual(["127.0.0.1:9100"]);
  expect(connectTo("192.168.1.20:9100", [wifi])).toEqual(["192.168.1.20:9100"]);
  expect(connectTo("0.0.0.0:9100", [wifi, { name: "eth0", address: "10.0.0.5" }])).toEqual(["127.0.0.1:9100", "192.168.1.20:9100", "10.0.0.5:9100"]);
});

test("a printer listens on this computer with a free port until the form says otherwise", () => {
  expect(resolveListen(DEFAULT_LISTEN)).toEqual({ valid: true, listen: "127.0.0.1:0" });
  expect(resolveListen({ host: "192.168.1.20", free: false, port: "9100" })).toEqual({ valid: true, listen: "192.168.1.20:9100" });
  expect(resolveListen({ host: "0.0.0.0", free: false, port: "" }).valid).toBe(false);
  expect(resolveListen({ host: "0.0.0.0", free: false, port: "70000" }).valid).toBe(false);
  expect(resolveListen({ host: "", free: true, port: "" }).valid).toBe(false);
});
