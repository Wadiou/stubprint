import type { Chunk, Connection, ConnectionChange } from "@/api/generated/types.gen";

/**
 * The list of connections as it is after a change that the live events told about, or nothing when
 * the list cannot be brought up to date from it, because it missed an earlier one: it has to be
 * asked for again. The pieces are not in the event, only how many there are and the one that just
 * came in, so they are kept only while each arrives in turn.
 */
export function applyChange(list: Connection[], change: ConnectionChange): Connection[] | undefined {
  const known = list.find((connection) => connection.id === change.id);
  const chunks = known?.chunks ?? [];
  let pieces: Chunk[];
  if (change.chunk) {
    if (chunks.length !== change.pieces - 1) return undefined;
    pieces = [...chunks, change.chunk];
  } else {
    // Bytes that were joined to the last piece, a reply, a receipt or the close: no new piece.
    if (chunks.length !== change.pieces) return undefined;
    pieces = chunks;
  }
  const next: Connection = {
    id: change.id,
    printer: change.printer,
    peer: change.peer,
    opened_at: change.opened_at,
    closed_at: change.closed_at,
    bytes_in: change.bytes_in,
    bytes_out: change.bytes_out,
    chunks: pieces,
    receipts: change.receipts,
  };
  return known
    ? list.map((connection) => (connection.id === change.id ? next : connection))
    : [...list, next];
}

/** A connection that has not closed yet. */
export const isOpen = (connection: Pick<Connection, "closed_at">) => connection.closed_at == null;

/** How many bytes, with thousands separated: `1,186 B`. */
export const bytesText = (bytes: number) => `${bytes.toLocaleString("en-US")} B`;

/** How long something took, in the unit that reads best: `40 ms`, `2.5 s`, `3 min 5 s`. */
export function durationText(ms: number) {
  if (ms < 1000) return `${ms} ms`;
  if (ms < 60_000) return `${Number((ms / 1000).toFixed(1))} s`;
  const seconds = Math.round(ms / 1000);
  return `${Math.floor(seconds / 60)} min ${seconds % 60} s`;
}

/** How long a connection lasted, or nothing while it is open. */
export const lasted = (connection: Pick<Connection, "opened_at" | "closed_at">) =>
  connection.closed_at == null ? undefined : connection.closed_at - connection.opened_at;

/** When each piece of bytes came in, one to a line: `+6 ms, 512 B`. */
export const piecesText = (chunks: Chunk[]) =>
  chunks.map((chunk) => `+${durationText(chunk.at)}, ${bytesText(chunk.bytes)}`).join("\n");
