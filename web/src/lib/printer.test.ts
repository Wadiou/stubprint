import { expect, test } from "vitest";
import { freeName, port, printerState } from "./printer";

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
