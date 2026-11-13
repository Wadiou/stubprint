import { useVirtualizer } from "@tanstack/react-virtual";
import { useEffect, useMemo, useRef, useState } from "react";
import type { ReceiptSummary } from "@/api/generated/types.gen";
import { ReceiptStateBadge } from "@/components/receipt-state";
import { Badge } from "@/components/ui/badge";
import { Input } from "@/components/ui/input";
import { Switch } from "@/components/ui/switch";
import { DIALECTS } from "@/lib/profile";

const ROW = 84;

/** The receipts that were printed, newest first. A new one is put at the top as it arrives. */
export function ReceiptRail({
  receipts,
  selected,
  following,
  onPick,
  onFollow,
}: {
  receipts: ReceiptSummary[];
  selected: number;
  following: boolean;
  onPick: (id: number) => void;
  onFollow: (on: boolean) => void;
}) {
  const [search, setSearch] = useState("");
  const shown = useMemo(() => {
    const wanted = search.trim().toLowerCase();
    const newest = [...receipts].reverse();
    return wanted ? newest.filter((receipt) => receipt.preview.join(" ").toLowerCase().includes(wanted)) : newest;
  }, [receipts, search]);

  const scroller = useRef<HTMLDivElement>(null);
  const virtual = useVirtualizer({
    count: shown.length,
    getScrollElement: () => scroller.current,
    estimateSize: () => ROW,
    overscan: 6,
  });

  // A list that is scrolled down keeps what is in view when receipts are put above it.
  const top = useRef(shown[0]?.id);
  useEffect(() => {
    const before = top.current;
    top.current = shown[0]?.id;
    const arrived = before === undefined ? -1 : shown.findIndex((receipt) => receipt.id === before);
    if (arrived > 0 && (scroller.current?.scrollTop ?? 0) > 0) {
      virtual.scrollToOffset((scroller.current?.scrollTop ?? 0) + arrived * ROW);
    }
  }, [shown, virtual]);

  return (
    <aside aria-label="Receipts" className="flex min-h-96 flex-col overflow-hidden rounded-xl border lg:h-[calc(100vh-8.5rem)]">
      <div className="flex items-center gap-2 px-4 pt-3">
        <span className="font-semibold">Receipts</span>
        <span className="text-muted-foreground">{receipts.length}</span>
        <div className="flex-1" />
        <span className="text-sm text-muted-foreground">Follow newest</span>
        <Switch size="sm" aria-label="Follow the newest receipt" checked={following} onCheckedChange={onFollow} />
      </div>
      <div className="p-4 pb-3">
        <Input
          aria-label="Search printed text"
          placeholder="Search printed text"
          value={search}
          onChange={(event) => setSearch(event.target.value)}
        />
      </div>
      <div ref={scroller} className="min-h-0 flex-1 overflow-auto border-t">
        {shown.length === 0 && <p className="p-4 text-sm text-muted-foreground">No receipt has this text.</p>}
        <div className="relative" style={{ height: virtual.getTotalSize() }}>
          {virtual.getVirtualItems().map((item) => {
            const receipt = shown[item.index];
            if (!receipt) return null;
            return (
              <button
                key={receipt.id}
                type="button"
                data-selected={receipt.id === selected}
                onClick={() => onPick(receipt.id)}
                className="absolute inset-x-0 flex flex-col justify-center gap-1 border-b px-4 text-left text-sm hover:bg-muted/50 data-[selected=true]:bg-muted"
                style={{ height: item.size, transform: `translateY(${item.start}px)` }}
              >
                <span className="flex items-baseline gap-2">
                  <span className="font-medium">#{receipt.id}</span>
                  <span className="flex-1" />
                  <span className="text-xs text-muted-foreground">{new Date(receipt.at).toLocaleTimeString()}</span>
                </span>
                <span className="truncate text-muted-foreground">{receipt.preview.slice(0, 2).join(", ") || "Nothing printed"}</span>
                <span className="flex gap-1.5">
                  <Badge variant="outline">{receipt.printer}</Badge>
                  <Badge variant="outline">{DIALECTS[receipt.dialect]}</Badge>
                  <ReceiptStateBadge state={receipt.state} />
                </span>
              </button>
            );
          })}
        </div>
      </div>
    </aside>
  );
}
