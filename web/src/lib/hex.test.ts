import { expect, test } from "vitest";
import { hexDump } from "./hex";

test("a dump has the offset, the bytes and the characters of each line of sixteen", () => {
  const bytes = Uint8Array.from([0x1b, 0x40, 0x48, 0x69, ...new Array(14).fill(0x0a)]);
  const [first, second] = hexDump(bytes).split("\n");
  expect(first).toBe(`00000000  1b 40 48 69 ${new Array(12).fill("0a").join(" ")}  .@Hi............`);
  expect(second).toBe(`00000010  0a 0a${" ".repeat(47 - 5)}  ..`);
});

test("nothing is dumped as nothing", () => {
  expect(hexDump(new Uint8Array())).toBe("");
});
