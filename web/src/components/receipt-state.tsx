import type { ReceiptSummary } from "@/api/generated/types.gen";
import { Badge } from "@/components/ui/badge";

/** Says when a receipt is not a finished one: the printer is still printing it, or it ended without a cut. A receipt that was cut says nothing. */
export function ReceiptStateBadge({ state }: { state: ReceiptSummary["state"] }) {
  if (state === "complete") return null;
  return state === "printing" ? <Badge>Printing</Badge> : <Badge variant="outline">Incomplete</Badge>;
}
