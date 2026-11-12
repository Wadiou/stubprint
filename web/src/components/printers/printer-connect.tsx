import { Copy } from "lucide-react";
import { toast } from "sonner";
import type { PrinterInfo } from "@/api/generated/types.gen";
import { Button } from "@/components/ui/button";
import { connectTo } from "@/lib/printer";
import { useNetwork } from "@/lib/use-network";
import { Field } from "./printer-fields";

/** The addresses to type into the program that prints here, each with a button to copy it. */
export function PrinterConnect({ printer }: { printer: PrinterInfo }) {
  const network = useNetwork();
  if (!network) return null;

  const copy = (address: string) => navigator.clipboard.writeText(address).then(() => toast.success(`Copied ${address}`));

  return (
    <Field label="Connect to">
      <div className="flex flex-col gap-1">
        {connectTo(printer.listen, network.addresses).map((address) => (
          <div key={address} className="flex items-center gap-2 text-sm">
            <span className="flex-1 font-mono text-sm">{address}</span>
            <Button variant="ghost" size="icon-sm" aria-label={`Copy ${address}`} onClick={() => copy(address)}>
              <Copy />
            </Button>
          </div>
        ))}
      </div>
    </Field>
  );
}
