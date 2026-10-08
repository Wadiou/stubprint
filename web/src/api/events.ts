import { useQueryClient } from "@tanstack/react-query";
import { useEffect, useState } from "react";
import {
  listConnectionsQueryKey,
  listPrintersQueryKey,
  listReceiptsQueryKey,
} from "./generated/@tanstack/react-query.gen";

// Each live event says which cached answers are out of date.
const STALE = {
  receipt: [listReceiptsQueryKey()],
  receipt_removed: [listReceiptsQueryKey()],
  cleared: [listReceiptsQueryKey()],
  status: [listPrintersQueryKey()],
  printer_added: [listPrintersQueryKey()],
  printer_changed: [listPrintersQueryKey()],
  printer_removed: [listPrintersQueryKey()],
  connection: [listConnectionsQueryKey()],
};

/** Follows the server's live events and refreshes what they change. True while connected. */
export function useLiveEvents() {
  const queryClient = useQueryClient();
  const [live, setLive] = useState(false);

  useEffect(() => {
    const source = new EventSource("/api/events");
    source.onopen = () => setLive(true);
    source.onerror = () => setLive(false);
    for (const [name, queryKeys] of Object.entries(STALE)) {
      source.addEventListener(name, () => {
        for (const queryKey of queryKeys) queryClient.invalidateQueries({ queryKey });
      });
    }
    return () => source.close();
  }, [queryClient]);

  return live;
}
