import { type QueryClient, useQueryClient } from "@tanstack/react-query";
import { useNavigate } from "@tanstack/react-router";
import { useEffect, useState } from "react";
import { toast } from "sonner";
import { applyChange } from "@/lib/connection";
import { stuckText } from "@/lib/printer";
import { readNotice } from "./notice";
import {
  listConnectionsOptions,
  listConnectionsQueryKey,
  listPrintersOptions,
  listPrintersQueryKey,
  listReceiptsQueryKey,
} from "./generated/@tanstack/react-query.gen";
import type { ConnectionChange, ReceiptSummary } from "./generated/types.gen";

// Each live event says which cached answers are out of date. A connection is not here: it comes
// with what it is now, which is put in the lists as it is.
const STALE = {
  receipt: [listReceiptsQueryKey()],
  receipt_removed: [listReceiptsQueryKey()],
  cleared: [listReceiptsQueryKey()],
  status: [listPrintersQueryKey()],
  held: [listPrintersQueryKey()],
  printer_added: [listPrintersQueryKey()],
  printer_changed: [listPrintersQueryKey()],
  printer_removed: [listPrintersQueryKey()],
};

/** Everything the events keep up to date, asked for again: after events were missed. */
function refreshAll(queryClient: QueryClient) {
  for (const queryKey of [listReceiptsQueryKey(), listPrintersQueryKey(), listConnectionsQueryKey()]) {
    queryClient.invalidateQueries({ queryKey });
  }
}

/** Puts what a connection is now in the two lists of connections the page asks for: all of them, and the ones of its printer. */
function putConnection(queryClient: QueryClient, change: ConnectionChange) {
  for (const { queryKey } of [listConnectionsOptions(), listConnectionsOptions({ query: { printer: change.printer } })]) {
    const list = queryClient.getQueryData(queryKey);
    // A list that is not here yet gets this when it is asked for.
    if (!list) continue;
    const next = applyChange(list, change);
    if (next) queryClient.setQueryData(queryKey, next);
    else queryClient.invalidateQueries({ queryKey });
  }
}

/** How long the page waits to ask again for a receipt that is growing, in milliseconds. */
const GROW_EVERY = 200;

/** The answers the page keeps about one receipt: the receipt itself, its text, picture, bytes and commands. */
const ABOUT_A_RECEIPT = ["getReceipt", "getReceiptText", "getReceiptPicture", "getReceiptBytes", "getTrace"];

/** Asks again for what the page shows of these receipts, and for the list they are in. */
function refreshReceipts(queryClient: QueryClient, ids: number[]) {
  queryClient.invalidateQueries({ queryKey: listReceiptsQueryKey() });
  queryClient.invalidateQueries({
    predicate: ({ queryKey }) => {
      const [key] = queryKey as [{ _id?: string; path?: { id?: number } } | undefined];
      return ABOUT_A_RECEIPT.includes(key?._id ?? "") && ids.includes(key?.path?.id ?? -1);
    },
  });
}

/** How long to wait before opening the stream again, after the browser gave up on it, in milliseconds. */
const RETRY_AFTER = 2000;

/** The first line of a receipt that has something written on it. */
const firstLine = (receipt: ReceiptSummary) => receipt.preview.find((line) => line.trim().length > 0)?.trim();

/** Follows the server's live events and keeps what they change up to date. True while connected. */
export function useLiveEvents() {
  const queryClient = useQueryClient();
  const navigate = useNavigate();
  const [live, setLive] = useState(false);

  useEffect(() => {
    let dropped = false;
    let retry: ReturnType<typeof setTimeout> | undefined;
    // The stream that is open now, which is a new one each time the browser gave up on the last.
    let current: EventSource | undefined;
    // The receipts that grew since the page asked for them last, and the time it waits before it does.
    const growing = new Set<number>();
    let flush: ReturnType<typeof setTimeout> | undefined;

    const connect = () => {
      const source = new EventSource("/api/events");
      current = source;
      source.onopen = () => {
        setLive(true);
        // What happened while it was not connected was not seen.
        if (dropped) refreshAll(queryClient);
        dropped = false;
      };
      source.onerror = () => {
        dropped = true;
        setLive(false);
        // A server that answers with an error, as the proxy does while the server is down, ends the
        // stream for good and the browser does not try again, so it is opened again after a moment.
        if (source.readyState === EventSource.CLOSED) retry = setTimeout(connect, RETRY_AFTER);
      };
      for (const [name, queryKeys] of Object.entries(STALE)) {
        source.addEventListener(name, () => {
          for (const queryKey of queryKeys) queryClient.invalidateQueries({ queryKey });
        });
      }
      source.addEventListener("receipt", (event) => {
        const notice = readNotice(event.data);
        if (notice?.type !== "receipt") return;
        const receipt = notice.value;
        // Its state changed, or it began: what is shown of it is not what it is now.
        refreshReceipts(queryClient, [receipt.id]);
        // It is told about when it begins, and not again when it ends.
        if (receipt.state !== "printing") return;
        toast.success(`Receipt #${receipt.id} on ${receipt.printer}`, {
          id: `receipt-${receipt.id}`,
          description: firstLine(receipt),
          action: { label: "See it", onClick: () => navigate({ to: "/", search: { receipt: receipt.id } }) },
        });
      });
      // A printer that cannot print holds what it is sent. This stays until it prints it.
      source.addEventListener("held", (event) => {
        const notice = readNotice(event.data);
        if (notice?.type !== "held") return;
        const { printer: name, held } = notice.value;
        const id = `held-${name}`;
        if (held === 0) {
          toast.dismiss(id);
          return;
        }
        const printer = queryClient.getQueryData(listPrintersOptions().queryKey)?.find((item) => item.name === name);
        toast.warning(`${name} cannot print`, {
          id,
          duration: Number.POSITIVE_INFINITY,
          description: printer ? stuckText(printer.status, held) : `${held.toLocaleString("en-US")} bytes waiting`,
        });
      });
      source.addEventListener("connection", (event) => {
        const notice = readNotice(event.data);
        // An event that is not understood leaves the lists wrong, so they are asked for again.
        if (notice?.type === "connection") putConnection(queryClient, notice.value);
        else queryClient.invalidateQueries({ queryKey: listConnectionsQueryKey() });
      });
      // A receipt that is printing grows, so what the page shows of it is asked for again, at most a few times a second.
      source.addEventListener("printing", (event) => {
        const notice = readNotice(event.data);
        if (notice?.type !== "printing") return;
        growing.add(notice.value.receipt);
        flush ??= setTimeout(() => {
          flush = undefined;
          refreshReceipts(queryClient, [...growing]);
          growing.clear();
        }, GROW_EVERY);
      });
      // The server says this one was too slow and missed some events.
      source.addEventListener("lagged", () => refreshAll(queryClient));
    };
    connect();
    return () => {
      clearTimeout(retry);
      clearTimeout(flush);
      current?.close();
    };
  }, [queryClient, navigate]);

  return live;
}
