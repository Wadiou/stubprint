import { expect, test } from "vitest";
import { readNotice } from "./notice";

const connection = {
  id: 1,
  printer: "till",
  peer: "192.168.1.42:51834",
  open: true,
  opened_at: 1_000,
  closed_at: null,
  bytes_in: 3,
  bytes_out: 0,
  pieces: 1,
  receipts: [],
  chunk: { at: 6, bytes: 3 },
};

test("a connection event is read as what the server describes", () => {
  const notice = readNotice(JSON.stringify({ type: "connection", value: connection }));
  expect(notice?.type).toBe("connection");
  if (notice?.type === "connection") expect(notice.value.chunk).toEqual({ at: 6, bytes: 3 });
});

test("an event that lacks a field or has a wrong one is not accepted", () => {
  const { pieces, ...without } = connection;
  expect(readNotice(JSON.stringify({ type: "connection", value: without }))).toBeUndefined();
  expect(readNotice(JSON.stringify({ type: "connection", value: { ...connection, bytes_in: "3" } }))).toBeUndefined();
  expect(readNotice(JSON.stringify({ type: "nothing", value: {} }))).toBeUndefined();
});

test("text that is not JSON is not accepted either", () => {
  expect(readNotice("not json")).toBeUndefined();
});
