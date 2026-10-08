import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { type FormEvent, useState } from "react";
import { addPrinterMutation, listPrintersQueryKey, listProfilesOptions } from "@/api/generated/@tanstack/react-query.gen";
import type { Dialect } from "@/api/generated/types.gen";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { Sheet, SheetBody, SheetContent, SheetFooter, SheetHeader, SheetTitle } from "@/components/ui/sheet";
import { DEFAULT_LISTEN, freeName, resolveListen } from "@/lib/printer";
import { DIALECTS } from "@/lib/profile";
import { usePrinters } from "@/lib/use-printers";
import { ModelPicker } from "./model-picker";
import { Field, refused } from "./printer-fields";
import { PrinterListenField } from "./printer-listen-field";

export interface PrinterPreset {
  model?: string;
  dialect?: Dialect;
}

function CreateForm({ preset, onAdded, onClose }: { preset?: PrinterPreset; onAdded?: (name: string) => void; onClose: () => void }) {
  const queryClient = useQueryClient();
  const printers = usePrinters();
  const models = useQuery({ ...listProfilesOptions(), staleTime: Number.POSITIVE_INFINITY }).data ?? [];
  const add = useMutation({
    ...addPrinterMutation(),
    onSuccess: (added) => {
      queryClient.invalidateQueries({ queryKey: listPrintersQueryKey() });
      onAdded?.(added.name);
      onClose();
    },
  });
  const [name, setName] = useState(() => freeName(printers.map((item) => item.name)));
  const [listen, setListen] = useState(DEFAULT_LISTEN);
  const [dialect, setDialect] = useState<Dialect>(preset?.dialect ?? "escpos");
  const [model, setModel] = useState(preset?.model ?? "default");

  const address = resolveListen(listen);

  const submit = (event: FormEvent) => {
    event.preventDefault();
    add.mutate({ body: { name: name || undefined, listen: address.listen, dialect, profile: model } });
  };

  return (
    <form onSubmit={submit} className="flex min-h-0 flex-1 flex-col">
      <SheetHeader>
        <SheetTitle>Add printer</SheetTitle>
      </SheetHeader>
      <SheetBody>
        <Field label="Name">
          <Input aria-label="Name" value={name} onChange={(event) => setName(event.target.value)} />
        </Field>
        <PrinterListenField value={listen} onChange={setListen} />
        <Field label="Language">
          <Select
            items={Object.entries(DIALECTS).map(([value, label]) => ({ value, label }))}
            value={dialect}
            onValueChange={(value) => setDialect(value as Dialect)}
          >
            <SelectTrigger aria-label="Language" className="w-full">
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              {Object.entries(DIALECTS).map(([value, label]) => (
                <SelectItem key={value} value={value}>
                  {label}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
        </Field>
        <Field label="Model">
          <ModelPicker models={models} value={model} dialect={dialect} onChange={setModel} />
        </Field>
        {add.isError && (
          <Alert variant="destructive">
            <AlertTitle>It was not added</AlertTitle>
            <AlertDescription>{refused(add.error)}</AlertDescription>
          </Alert>
        )}
      </SheetBody>
      <SheetFooter>
        <div className="flex-1" />
        <Button type="button" variant="ghost" onClick={onClose}>
          Cancel
        </Button>
        <Button type="submit" disabled={add.isPending || !address.valid}>
          Add printer
        </Button>
      </SheetFooter>
    </form>
  );
}

/** Makes a printer, at the side of the table. The form is made again each time it opens, with the model and the language that are given. */
export function PrinterCreateSheet({
  open,
  preset,
  onAdded,
  onClose,
}: {
  open: boolean;
  preset?: PrinterPreset;
  /** Told the name of the printer that was made. */
  onAdded?: (name: string) => void;
  onClose: () => void;
}) {
  return (
    <Sheet open={open} onOpenChange={(next) => !next && onClose()}>
      <SheetContent className="sm:max-w-md">{open && <CreateForm preset={preset} onAdded={onAdded} onClose={onClose} />}</SheetContent>
    </Sheet>
  );
}
