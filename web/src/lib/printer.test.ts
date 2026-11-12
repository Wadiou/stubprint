import { expect, test } from "vitest";
import { byteSize, connectTo, DEFAULT_LISTEN, freeName, heldText, leftText, MAX_ROLL, MAX_SPEED, metresText, port, printerState, printSpeed, resolveListen, rollLength, rollShare, stuckText } from "./printer";

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

test("the size a printer holds is a whole number of bytes from 1 to the most", () => {
  expect(byteSize("4096")).toBe(4096);
  expect(byteSize("1")).toBe(1);
  expect(byteSize("1048576")).toBe(1_048_576);
  for (const bad of ["", "0", "-5", "12.5", "1e3", "abc", "1048577", " 10"]) expect(byteSize(bad)).toBeUndefined();
});

test("how full the buffer is says the bytes held and the size, with thousands separated", () => {
  expect(heldText(0, 4096)).toBe("Holding 0 / 4,096 bytes");
  expect(heldText(312, 4096)).toBe("Holding 312 / 4,096 bytes");
});

test("a roll is typed in metres and is asked for in millimetres", () => {
  expect(rollLength("80")).toBe(80_000);
  expect(rollLength("1.5")).toBe(1500);
  expect(rollLength("0,25")).toBe(250);
  expect(rollLength(" 12.345 ")).toBe(12_345);
  expect(rollLength("0")).toBeUndefined();
  expect(rollLength("1.2345")).toBeUndefined();
  expect(rollLength("-3")).toBeUndefined();
  expect(rollLength("abc")).toBeUndefined();
  expect(rollLength("")).toBeUndefined();
  expect(rollLength(String(MAX_ROLL / 1000 + 1))).toBeUndefined();
});

test("paper is told in metres, and how much of the roll is left is a share of it", () => {
  expect(metresText(80_000)).toBe("80");
  expect(metresText(42_300)).toBe("42.3");
  expect(metresText(250)).toBe("0.25");
  expect(leftText(42_300, 80_000)).toBe("42.3 m of 80 m");
  expect(rollShare(20_000, 80_000)).toBe(0.25);
  expect(rollShare(0, 80_000)).toBe(0);
  expect(rollShare(5, 0)).toBe(0);
  expect(rollShare(90_000, 80_000)).toBe(1);
});

test("a speed is typed in millimetres a second, from 1 to the most", () => {
  expect(printSpeed("150")).toBe(150);
  expect(printSpeed(" 80 ")).toBe(80);
  expect(printSpeed("0")).toBeUndefined();
  expect(printSpeed("1.5")).toBeUndefined();
  expect(printSpeed("-3")).toBeUndefined();
  expect(printSpeed("fast")).toBeUndefined();
  expect(printSpeed("")).toBeUndefined();
  expect(printSpeed(String(MAX_SPEED + 1))).toBeUndefined();
});
