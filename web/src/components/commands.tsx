import { useVirtualizer } from "@tanstack/react-virtual";
import { useEffect, useRef } from "react";

export type Row = { index: number; label: string; detail: string; start: number; end: number; hex: string };

const ROW = 38;

/** The commands of a receipt, one line each. Only the lines in view are made. */
export function CommandList({
  rows,
  selected,
  onSelect,
}: {
  rows: Row[];
  selected: number;
  onSelect: (index: number) => void;
}) {
  const scroller = useRef<HTMLDivElement>(null);
  const virtual = useVirtualizer({
    count: rows.length,
    getScrollElement: () => scroller.current,
    estimateSize: () => ROW,
    overscan: 8,
  });
  const at = rows.findIndex((row) => row.index === selected);
  useEffect(() => {
    if (at >= 0) virtual.scrollToIndex(at);
  }, [at, virtual]);

  return (
    <div ref={scroller} className="h-96 overflow-auto rounded-md border">
      <div className="relative" style={{ height: virtual.getTotalSize() }}>
        {virtual.getVirtualItems().map((item) => {
          const row = rows[item.index];
          if (!row) return null;
          return (
            <button
              key={row.index}
              type="button"
              data-selected={row.index === selected}
              onClick={() => onSelect(row.index)}
              className="absolute inset-x-0 flex items-center gap-3 border-b px-3 text-left text-sm hover:bg-muted/50 data-[selected=true]:bg-muted"
              style={{ height: item.size, transform: `translateY(${item.start}px)` }}
            >
              <span className="w-8 text-right text-xs text-muted-foreground">{row.index + 1}</span>
              <span className="min-w-0 flex-1 truncate">
                <span className="font-medium">{row.label}</span>{" "}
                <span className="text-muted-foreground">{row.detail}</span>
              </span>
              <span className="max-w-28 truncate font-mono text-xs text-muted-foreground">{row.hex}</span>
            </button>
          );
        })}
      </div>
    </div>
  );
}
