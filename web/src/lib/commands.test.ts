import { expect, test } from "vitest";
import type { Event } from "@/api/generated/types.gen";
import { snapshot as state } from "@/test/fixtures";
import { describe as describeEvent, stateRows } from "./commands";

test("a command is named in words with what it asked for", () => {
  const text: Event = { type: "text", value: { align: "center", style: state.style, text: "Green Market" } };
  expect(describeEvent(text)).toEqual({ label: "Text", detail: '"Green Market"' });
  expect(describeEvent({ type: "align", value: "center" })).toEqual({ label: "Align", detail: "centre" });
  expect(describeEvent({ type: "cut", value: "partial" })).toEqual({ label: "Cut", detail: "partial" });
  expect(describeEvent({ type: "feed", value: { amount: { lines: 3 }, reverse: true } }).detail).toBe(
    "3 lines backwards",
  );
  expect(describeEvent({ type: "unknown", value: { bytes: "1b 1d 78", offset: 4 } }).label).toBe("Not read");
});

test("the state is listed with the style the next text would have", () => {
  const rows = Object.fromEntries(stateRows(state).map((row) => [row.name, row.value]));
  expect(rows.Size).toBe("2 × 2");
  expect(rows.Bold).toBe("on");
  expect(rows.Align).toBe("centre");
  expect(rows["Code page"]).toBe("CP437");
  expect(rows["Print width"]).toBe("the rest of the paper");
});
