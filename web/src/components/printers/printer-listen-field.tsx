import { ChevronDown } from "lucide-react";
import { Button } from "@/components/ui/button";
import { DropdownMenu, DropdownMenuContent, DropdownMenuItem, DropdownMenuTrigger } from "@/components/ui/dropdown-menu";
import { Input } from "@/components/ui/input";
import { Switch } from "@/components/ui/switch";
import type { ListenChoice } from "@/lib/printer";
import { useNetwork } from "@/lib/use-network";
import { Field } from "./printer-fields";

/** The address a printer listens on and its port. The address is typed, or taken from the ones this computer has. */
export function PrinterListenField({ value, onChange }: { value: ListenChoice; onChange: (value: ListenChoice) => void }) {
  const hosts = ["127.0.0.1", "0.0.0.0", ...(useNetwork()?.addresses ?? []).map((entry) => entry.address)];

  return (
    <Field label="Address">
      <div className="flex items-center gap-2">
        <div className="flex flex-1 items-center gap-1">
          <Input aria-label="Address" className="font-mono" value={value.host} onChange={(event) => onChange({ ...value, host: event.target.value.trim() })} />
          <DropdownMenu>
            <DropdownMenuTrigger render={<Button type="button" variant="outline" size="icon" aria-label="Addresses of this computer" />}>
              <ChevronDown />
            </DropdownMenuTrigger>
            <DropdownMenuContent align="end">
              {hosts.map((host) => (
                <DropdownMenuItem key={host} className="font-mono" onClick={() => onChange({ ...value, host })}>
                  {host}
                </DropdownMenuItem>
              ))}
            </DropdownMenuContent>
          </DropdownMenu>
        </div>
        <span>:</span>
        <Input
          aria-label="Port"
          className="w-24 font-mono"
          inputMode="numeric"
          placeholder="Auto"
          disabled={value.free}
          value={value.free ? "" : value.port}
          onChange={(event) => onChange({ ...value, port: event.target.value.trim() })}
        />
      </div>
      <div className="flex items-center gap-3">
        <Switch aria-label="Pick a free port" checked={value.free} onCheckedChange={(free) => onChange({ ...value, free })} />
        <span className="text-sm">Pick a free port</span>
      </div>
    </Field>
  );
}
