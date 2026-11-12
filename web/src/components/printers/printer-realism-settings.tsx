import { useMutation, useQueryClient } from "@tanstack/react-query";
import { useState } from "react";
import { listPrintersQueryKey, updatePrinterMutation } from "@/api/generated/@tanstack/react-query.gen";
import type { PrinterInfo } from "@/api/generated/types.gen";
import { Input } from "@/components/ui/input";
import { Switch } from "@/components/ui/switch";
import { DEFAULT_HOLD, heldText, holdSize, MAX_HOLD } from "@/lib/printer";

/** The receive buffer of the printer, which holds what it is sent while it cannot print. On by default, and 0 turns it off. */
export function PrinterRealismSettings({ printer }: { printer: PrinterInfo }) {
  const queryClient = useQueryClient();
  const update = useMutation({
    ...updatePrinterMutation(),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: listPrintersQueryKey() }),
  });
  const [text, setText] = useState(String(printer.hold_offline || DEFAULT_HOLD));
  const size = holdSize(text);
  const on = printer.hold_offline > 0;
  const hold = (bytes: number) => update.mutate({ path: { name: printer.name }, body: { hold_offline: bytes } });

  return (
    <section className="flex flex-col">
      <h3 className="mb-3 font-semibold">Receive buffer</h3>
      <div className="flex flex-col gap-2 border-t pt-3">
        <div className="flex items-center gap-3">
          <span className="flex-1 font-medium">Hold data while it cannot print</span>
          <Input
            aria-label="Bytes held while it cannot print"
            aria-invalid={size === undefined}
            inputMode="numeric"
            className="h-8 w-24 text-right"
            value={text}
            onChange={(event) => setText(event.target.value)}
            // A size that is typed while the switch is on applies when the field is left.
            onBlur={() => {
              if (on && size !== undefined && size !== printer.hold_offline) hold(size);
            }}
          />
          <span className="w-10 text-sm text-muted-foreground">bytes</span>
          <Switch
            aria-label="Hold data while it cannot print"
            checked={on}
            disabled={!on && size === undefined}
            onCheckedChange={(next) => hold(next ? (size ?? DEFAULT_HOLD) : 0)}
          />
        </div>
        {on && <p className="text-sm text-muted-foreground">{heldText(printer.held, printer.hold_offline)}</p>}
        {size === undefined && <p className="text-sm text-destructive">Use a whole number of bytes from 1 to {MAX_HOLD.toLocaleString("en-US")}.</p>}
      </div>
    </section>
  );
}
