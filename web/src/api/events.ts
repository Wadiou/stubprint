import { type QueryClient, useQueryClient } from "@tanstack/react-query";
import { useEffect, useState } from "react";
import { applyChange } from "@/lib/connection";
import { readNotice } from "./notice";
import {
  listConnectionsOptions,
  listConnectionsQueryKey,
  listPrintersQueryKey,
  listReceiptsQueryKey,
} from "./generated/@tanstack/react-query.gen";
import type { ConnectionChange } from "./generated/types.gen";

// Each live event says which cached answers are out of date. A connection is not here: it comes
// with what it is now, which is put in the lists as it is.
const STALE = {
  receipt: [listReceiptsQueryKey()],
  receipt_removed: [listReceiptsQueryKey()],
  cleared: [listReceiptsQueryKey()],
  status: [listPrintersQueryKey()],
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

/** Follows the server's live events and keeps what they change up to date. True while connected. */
export function useLiveEvents() {
  const queryClient = useQueryClient();
  const [live, setLive] = useState(false);

  useEffect(() => {
    const source = new EventSource("/api/events");
    let dropped = false;
    source.onopen = () => {
      setLive(true);
      // What happened while it was not connected was not seen.
      if (dropped) refreshAll(queryClient);
      dropped = false;
    };
    source.onerror = () => {
      dropped = true;
      setLive(false);
    };
    for (const [name, queryKeys] of Object.entries(STALE)) {
      source.addEventListener(name, () => {
        for (const queryKey of queryKeys) queryClient.invalidateQueries({ queryKey });
      });
    }
    source.addEventListener("connection", (event) => {
      const notice = readNotice(event.data);
      // An event that is not understood leaves the lists wrong, so they are asked for again.
      if (notice?.type === "connection") putConnection(queryClient, notice.value);
      else queryClient.invalidateQueries({ queryKey: listConnectionsQueryKey() });
    });
    // The server says this one was too slow and missed some events.
    source.addEventListener("lagged", () => refreshAll(queryClient));
    return () => source.close();
  }, [queryClient]);

  return live;
}
