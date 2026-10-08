import { Link } from "@tanstack/react-router";
import { ChevronDown, Plus } from "lucide-react";
import { Button } from "@/components/ui/button";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuRadioGroup,
  DropdownMenuRadioItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { useActivePrinter } from "@/lib/active-printer";
import { port, printerState, type Tone } from "@/lib/printer";
import { DIALECTS } from "@/lib/profile";

const DOT: Record<Tone, string> = { ok: "bg-green-500", warn: "bg-amber-500", bad: "bg-red-500" };

/** The printer the pages are about. Its menu chooses another one, or goes to add one. */
export function PrinterPicker() {
  const { printer, printers, choose } = useActivePrinter();

  if (!printer) {
    return (
      <Button variant="outline" size="sm" render={<Link to="/printers" search={{ add: true }} />}>
        <Plus />
        Add printer
      </Button>
    );
  }
  return (
    <DropdownMenu>
      <DropdownMenuTrigger render={<Button variant="outline" size="sm" aria-label="Printer" />}>
        <span className={`size-2 rounded-full ${DOT[printerState(printer.status).tone]}`} />
        {printer.name}
        <span className="font-normal text-muted-foreground">
          {DIALECTS[printer.dialect]}, {port(printer)}
        </span>
        <ChevronDown className="text-muted-foreground" />
      </DropdownMenuTrigger>
      <DropdownMenuContent align="end" className="min-w-64">
        <DropdownMenuRadioGroup value={printer.name} onValueChange={(name) => choose(String(name))}>
          {printers.map((item) => {
            const state = printerState(item.status);
            return (
              <DropdownMenuRadioItem key={item.name} value={item.name}>
                <span className={`size-2 rounded-full ${DOT[state.tone]}`} />
                <span className="flex-1">{item.name}</span>
                <span className="text-xs text-muted-foreground">
                  {DIALECTS[item.dialect]}, {port(item)}
                </span>
                <span className="sr-only">{state.label}</span>
              </DropdownMenuRadioItem>
            );
          })}
        </DropdownMenuRadioGroup>
        <DropdownMenuSeparator />
        <DropdownMenuItem render={<Link to="/printers" search={{ add: true }} />}>
          <Plus />
          Add printer
        </DropdownMenuItem>
      </DropdownMenuContent>
    </DropdownMenu>
  );
}
