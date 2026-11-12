import { expect, test } from "vitest";
import { liveSearch, printersSearch } from "./search";

test("the live paper takes a receipt number and drops anything else", () => {
  expect(liveSearch.parse({ receipt: 3 })).toEqual({ receipt: 3 });
  expect(liveSearch.parse({ receipt: "x" })).toEqual({});
  expect(liveSearch.parse({})).toEqual({});
});

test("the printers take a printer, a printer to remove, and add said as true or as the word", () => {
  expect(printersSearch.parse({ printer: "front", remove: "bar" })).toEqual({ printer: "front", remove: "bar" });
  expect(printersSearch.parse({ add: true })).toEqual({ add: true });
  expect(printersSearch.parse({ add: "true" })).toEqual({ add: true });
  expect(printersSearch.parse({ add: "no" })).toEqual({});
  expect(printersSearch.parse({ printer: 4 })).toEqual({});
});

test("what the page does not know is left out", () => {
  expect(printersSearch.parse({ printer: "front", other: 1 })).toEqual({ printer: "front" });
});
