import { useMutation, useQueryClient } from "@tanstack/react-query";
import { useState } from "react";
import { listPrintersQueryKey, updatePrinterMutation } from "@/api/generated/@tanstack/react-query.gen";
import type { PrinterInfo } from "@/api/generated/types.gen";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Switch } from "@/components/ui/switch";
import { DEFAULT_ROLL, leftText, MAX_ROLL, metresText, rollLength, rollShare } from "@/lib/printer";

const LEVEL = { adequate: "bg-primary", near_end: "bg-amber-500", end: "bg-destructive" } as const;

/** The paper roll: how long it is, how much is left, and a new one. A printer with no roll never runs out, and its paper is set by hand. */
export function PrinterPaper({ printer }: { printer: PrinterInfo }) {
  const queryClient = useQueryClient();
  const update = useMutation({
    ...updatePrinterMutation(),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: listPrintersQueryKey() }),
  });
  const put = (millimetres: number) => update.mutate({ path: { name: printer.name }, body: { paper_roll: millimetres } });
  const [text, setText] = useState(metresText(printer.paper_roll || DEFAULT_ROLL));
  const length = rollLength(text);
  const on = printer.paper_roll > 0;
  const share = rollShare(printer.paper_left, printer.paper_roll);

  return (
    <section className="flex flex-col gap-3">
      <h3 className="font-semibold">Paper roll</h3>
      <div className="flex items-center gap-3">
        <div className="flex flex-1 flex-col">
          <span className="font-medium">Roll length</span>
          <span className="text-sm text-muted-foreground">It runs down as receipts print, and a new length puts in a new roll</span>
        </div>
        <Input
          aria-label="Roll length in metres"
          aria-invalid={length === undefined}
          inputMode="decimal"
          className="h-8 w-24 text-right"
          value={text}
          onChange={(event) => setText(event.target.value)}
          onBlur={() => {
            if (on && length !== undefined && length !== printer.paper_roll) put(length);
          }}
        />
        <span className="w-10 text-sm text-muted-foreground">m</span>
        <Switch aria-label="Paper roll" checked={on} disabled={!on && length === undefined} onCheckedChange={(next) => put(next ? (length ?? DEFAULT_ROLL) : 0)} />
      </div>
      {length === undefined && <p className="text-sm text-destructive">Use a length in metres, up to {MAX_ROLL / 1000}, with up to three decimals.</p>}
      {on && (
        <div className="flex flex-col gap-2">
          <div className="flex items-center justify-between gap-3 text-sm">
            <span className="text-muted-foreground">Paper left</span>
            <span className="font-medium">{leftText(printer.paper_left, printer.paper_roll)}</span>
          </div>
          <div
            role="progressbar"
            aria-label="Paper left"
            aria-valuemin={0}
            aria-valuemax={printer.paper_roll}
            aria-valuenow={Math.round(printer.paper_left)}
            className="h-2 overflow-hidden rounded-full bg-muted"
          >
            <div className={`h-full rounded-full ${LEVEL[printer.status.paper]}`} style={{ width: `${share * 100}%` }} />
          </div>
          <Button variant="outline" size="sm" className="self-start" onClick={() => put(printer.paper_roll)}>
            New roll
          </Button>
        </div>
      )}
    </section>
  );
}
