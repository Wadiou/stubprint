import { useMutation, useQueryClient } from "@tanstack/react-query";
import { useState } from "react";
import { listPrintersQueryKey, updatePrinterMutation } from "@/api/generated/@tanstack/react-query.gen";
import type { PrinterInfo } from "@/api/generated/types.gen";
import { Input } from "@/components/ui/input";
import { Switch } from "@/components/ui/switch";
import { byteSize, DEFAULT_HOLD, DEFAULT_RECEIVE, MAX_BYTES } from "@/lib/printer";

/** A setting that is a number of bytes and is off at 0: the size, and a switch. A size typed while it is on applies when the field is left. */
function ByteSetting({ label, description, bytes, fallback, apply }: { label: string; description: string; bytes: number; fallback: number; apply: (bytes: number) => void }) {
  const [text, setText] = useState(String(bytes || fallback));
  const size = byteSize(text);
  const on = bytes > 0;

  return (
    <div className="flex flex-col gap-2 border-t pt-3 first:border-t-0 first:pt-0">
      <div className="flex items-center gap-3">
        <div className="flex flex-1 flex-col">
          <span className="font-medium">{label}</span>
          <span className="text-sm text-muted-foreground">{description}</span>
        </div>
        <Input
          aria-label={`Bytes: ${label}`}
          aria-invalid={size === undefined}
          inputMode="numeric"
          className="h-8 w-24 text-right"
          value={text}
          onChange={(event) => setText(event.target.value)}
          onBlur={() => {
            if (on && size !== undefined && size !== bytes) apply(size);
          }}
        />
        <span className="w-10 text-sm text-muted-foreground">bytes</span>
        <Switch aria-label={label} checked={on} disabled={!on && size === undefined} onCheckedChange={(next) => apply(next ? (size ?? fallback) : 0)} />
      </div>
      {size === undefined && <p className="text-sm text-destructive">Use a whole number of bytes from 1 to {MAX_BYTES.toLocaleString("en-US")}.</p>}
    </div>
  );
}

/** The buffers of the printer: what it holds while it cannot print, and what the network keeps for a program. */
export function PrinterRealismSettings({ printer }: { printer: PrinterInfo }) {
  const queryClient = useQueryClient();
  const update = useMutation({
    ...updatePrinterMutation(),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: listPrintersQueryKey() }),
  });
  const change = (body: { hold_offline?: number; receive_buffer?: number }) => update.mutate({ path: { name: printer.name }, body });

  return (
    <section className="flex flex-col gap-3">
      <h3 className="font-semibold">Receive buffer</h3>
      <ByteSetting
        key={`hold-${printer.hold_offline}`}
        label="Printer buffer"
        description="Bytes kept while it cannot print"
        bytes={printer.hold_offline}
        fallback={DEFAULT_HOLD}
        apply={(bytes) => change({ hold_offline: bytes })}
      />
      <ByteSetting
        key={`receive-${printer.receive_buffer}`}
        label="Network buffer"
        description="Bytes the network keeps for a program"
        bytes={printer.receive_buffer}
        fallback={DEFAULT_RECEIVE}
        apply={(bytes) => change({ receive_buffer: bytes })}
      />
    </section>
  );
}
