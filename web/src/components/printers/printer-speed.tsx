import { useMutation, useQueryClient } from "@tanstack/react-query";
import { useState } from "react";
import { listPrintersQueryKey, updatePrinterMutation } from "@/api/generated/@tanstack/react-query.gen";
import type { PrinterInfo } from "@/api/generated/types.gen";
import { Input } from "@/components/ui/input";
import { Switch } from "@/components/ui/switch";
import { DEFAULT_SPEED, MAX_SPEED, printSpeed } from "@/lib/printer";

/** How fast the printer prints. A receipt keeps it busy while its paper comes out, and a program that sends more waits. Off, it takes no time. A speed typed while it is on applies when the field is left. */
export function PrinterSpeed({ printer }: { printer: PrinterInfo }) {
  const queryClient = useQueryClient();
  const update = useMutation({
    ...updatePrinterMutation(),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: listPrintersQueryKey() }),
  });
  const put = (speed: number) => update.mutate({ path: { name: printer.name }, body: { print_speed: speed } });
  const [text, setText] = useState(String(printer.print_speed || DEFAULT_SPEED));
  const speed = printSpeed(text);
  const on = printer.print_speed > 0;

  return (
    <section className="flex flex-col gap-3">
      <h3 className="font-semibold">Speed</h3>
      <div className="flex items-center gap-3">
        <div className="flex flex-1 flex-col">
          <span className="font-medium">Print speed</span>
          <span className="text-sm text-muted-foreground">Each receipt keeps it busy while its paper comes out</span>
        </div>
        <Input
          aria-label="Print speed in millimetres a second"
          aria-invalid={speed === undefined}
          inputMode="numeric"
          className="h-8 w-24 text-right"
          value={text}
          onChange={(event) => setText(event.target.value)}
          onBlur={() => {
            if (on && speed !== undefined && speed !== printer.print_speed) put(speed);
          }}
        />
        <span className="w-10 text-sm text-muted-foreground">mm/s</span>
        <Switch aria-label="Print speed" checked={on} disabled={!on && speed === undefined} onCheckedChange={(next) => put(next ? (speed ?? DEFAULT_SPEED) : 0)} />
      </div>
      {speed === undefined && <p className="text-sm text-destructive">Use a whole number of millimetres a second from 1 to {MAX_SPEED}.</p>}
    </section>
  );
}
