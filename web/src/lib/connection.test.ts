import { expect, test } from "vitest";
import type { Connection, ConnectionChange } from "@/api/generated/types.gen";
import { applyChange, bytesText, durationText, isOpen, lasted, piecesText } from "./connection";

const opened: ConnectionChange = {
  id: 1,
  printer: "till",
  peer: "192.168.1.42:51834",
  open: true,
  opened_at: 1_000,
  closed_at: null,
  bytes_in: 0,
  bytes_out: 0,
  pieces: 0,
  receipts: [],
};

test("a connection is open until it has a time of closing", () => {
  expect(isOpen({ closed_at: null })).toBe(true);
  expect(isOpen({})).toBe(true);
  expect(isOpen({ closed_at: 1_000 })).toBe(false);
});

test("bytes are written with their thousands separated", () => {
  expect(bytesText(0)).toBe("0 B");
  expect(bytesText(1186)).toBe("1,186 B");
});

test("a duration is given in the unit that reads best", () => {
  expect(durationText(40)).toBe("40 ms");
  expect(durationText(2500)).toBe("2.5 s");
  expect(durationText(12_000)).toBe("12 s");
  expect(durationText(185_000)).toBe("3 min 5 s");
});

test("a connection lasts from its opening to its closing, and has no length while it is open", () => {
  expect(lasted({ opened_at: 1_000, closed_at: 1_040 })).toBe(40);
  expect(lasted({ opened_at: 1_000, closed_at: null })).toBeUndefined();
});

test("the pieces are told one to a line, with the time each came in", () => {
  expect(piecesText([])).toBe("");
  expect(
    piecesText([
      { at: 0, bytes: 512 },
      { at: 6, bytes: 128 },
    ]),
  ).toBe("+0 ms, 512 B\n+6 ms, 128 B");
});

/** The list after a change that has to be applicable: the test fails when it is not. */
function applied(list: Connection[], change: ConnectionChange) {
  const next = applyChange(list, change);
  if (!next) throw new Error("The change could not be applied to the list.");
  return next;
}

test("a connection that opens is added to the list, and each change after it is put in its place", () => {
  const first = applied([], opened);
  expect(first).toEqual([
    {
      id: 1,
      printer: "till",
      peer: "192.168.1.42:51834",
      opened_at: 1_000,
      closed_at: null,
      bytes_in: 0,
      bytes_out: 0,
      chunks: [],
      receipts: [],
    },
  ]);

  const got = applied(first, { ...opened, bytes_in: 3, pieces: 1, chunk: { at: 6, bytes: 3 } });
  expect(got[0]?.bytes_in).toBe(3);
  expect(got[0]?.chunks).toEqual([{ at: 6, bytes: 3 }]);

  const printed = applied(got, { ...opened, bytes_in: 7, pieces: 2, chunk: { at: 20, bytes: 4 }, receipts: [1] });
  expect(printed[0]?.chunks).toHaveLength(2);
  expect(printed[0]?.receipts).toEqual([1]);

  const closed = applied(printed, { ...opened, bytes_in: 7, pieces: 2, receipts: [1], open: false, closed_at: 1_040 });
  expect(closed[0]?.closed_at).toBe(1_040);
  expect(closed[0]?.chunks).toHaveLength(2);
});

test("the other connections stay as they were", () => {
  const first = applied([], opened);
  const both = applied(first, { ...opened, id: 2, peer: "http" });
  expect(both.map((connection) => connection.id)).toEqual([1, 2]);
  const changed = applied(both, { ...opened, id: 2, peer: "http", bytes_out: 12 });
  expect(changed[0]).toBe(both[0]);
  expect(changed[1]?.bytes_out).toBe(12);
});

test("a change that cannot follow what the list has says the list must be asked for again", () => {
  const first = applied([], opened);
  // A piece was missed: the second one came while the list has none.
  expect(applyChange(first, { ...opened, bytes_in: 7, pieces: 2, chunk: { at: 20, bytes: 4 } })).toBeUndefined();
  // A connection the list never saw, which already has pieces.
  expect(applyChange([], { ...opened, id: 9, pieces: 3 })).toBeUndefined();
});

test("the list keeps the latest 200, as the server does", () => {
  let list: Connection[] = [];
  for (let id = 1; id <= 201; id++) list = applied(list, { ...opened, id });
  expect(list).toHaveLength(200);
  expect(list[0]?.id).toBe(2);
});
