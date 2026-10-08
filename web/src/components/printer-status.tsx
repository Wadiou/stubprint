import { Link } from "@tanstack/react-router";
import { Plus } from "lucide-react";
import { useState } from "react";
import { Button } from "@/components/ui/button";
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover";
import { Separator } from "@/components/ui/separator";
import { port, printerState, type Tone } from "@/lib/printer";
import { DIALECTS } from "@/lib/profile";
import { usePrinters } from "@/lib/use-printers";

const DOT: Record<Tone, string> = { ok: "bg-green-500", warn: "bg-amber-500", bad: "bg-red-500" };
const WORST: Tone[] = ["ok", "warn", "bad"];

/** A dot that is lit while the printer is fine, and pulses to say it is listening. */
function Dot({ tone }: { tone: Tone }) {
  return (
    <span className="relative flex size-2 shrink-0">
      {tone === "ok" && <span className="absolute inline-flex size-full animate-ping rounded-full bg-green-500 opacity-75" />}
      <span className={`relative inline-flex size-2 rounded-full ${DOT[tone]}`} />
    </span>
  );
}

/** The printers that are working, all of them, and how each is doing, with a way to add another. */
export function PrinterStatus() {
  const printers = usePrinters();
  const [open, setOpen] = useState(false);
  const states = printers.map((printer) => ({ printer, state: printerState(printer.status) }));
  const worst = states.reduce<Tone>((worse, { state }) => (WORST.indexOf(state.tone) > WORST.indexOf(worse) ? state.tone : worse), "ok");

  if (printers.length === 0) {
    return (
      <Button variant="outline" size="sm" render={<Link to="/printers" search={{ add: true }} />}>
        <Plus />
        Add printer
      </Button>
    );
  }
  return (
    <Popover open={open} onOpenChange={setOpen}>
      <PopoverTrigger render={<Button variant="outline" size="sm" aria-label="Printers" />}>
        <Dot tone={worst} />
        {printers.length === 1 ? "1 printer" : `${printers.length} printers`}
      </PopoverTrigger>
      <PopoverContent className="w-80 p-2">
        <div className="flex flex-col">
          {states.map(({ printer, state }) => (
            <Link
              key={printer.name}
              to="/printers"
              search={{ printer: printer.name }}
              onClick={() => setOpen(false)}
              className="flex items-center gap-2 rounded-md px-2 py-2 text-sm hover:bg-muted"
            >
              <Dot tone={state.tone} />
              <span className="flex-1 font-medium">{printer.name}</span>
              <span className="text-xs text-muted-foreground">
                {DIALECTS[printer.dialect]}, :{port(printer)}
              </span>
              <span className="sr-only">{state.label}</span>
            </Link>
          ))}
        </div>
        <Separator className="my-2" />
        <Link
          to="/printers"
          search={{ add: true }}
          onClick={() => setOpen(false)}
          className="flex items-center gap-2 rounded-md px-2 py-2 text-sm hover:bg-muted"
        >
          <Plus className="size-4" />
          Add printer
        </Link>
      </PopoverContent>
    </Popover>
  );
}
