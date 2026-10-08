import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { type FormEvent, useState } from "react";
import { addPrinterMutation, listPrintersQueryKey, listProfilesOptions, updatePrinterMutation } from "@/api/generated/@tanstack/react-query.gen";
import type { Dialect, PrinterInfo } from "@/api/generated/types.gen";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { Sheet, SheetBody, SheetContent, SheetDescription, SheetFooter, SheetHeader, SheetTitle } from "@/components/ui/sheet";
import { useActivePrinter } from "@/lib/active-printer";
import { DIALECTS, paperText } from "@/lib/profile";
import { Field, ModelSelect, refused } from "./printer-fields";
import { PrinterStatusSettings } from "./printer-status-settings";

export type PrinterMutateMode = "create" | "edit";

function MutateForm({
  mode,
  printer,
  onClose,
  onRemove,
}: {
  mode: PrinterMutateMode;
  printer?: PrinterInfo;
  onClose: () => void;
  onRemove: (printer: PrinterInfo) => void;
}) {
  const queryClient = useQueryClient();
  const { choose } = useActivePrinter();
  const models = useQuery({ ...listProfilesOptions(), staleTime: Number.POSITIVE_INFINITY }).data ?? [];
  const editing = mode === "edit" && printer !== undefined;
  const refresh = () => queryClient.invalidateQueries({ queryKey: listPrintersQueryKey() });
  const add = useMutation({
    ...addPrinterMutation(),
    onSuccess: (added) => {
      refresh();
      choose(added.name);
      onClose();
    },
  });
  const update = useMutation({
    ...updatePrinterMutation(),
    onSuccess: () => {
      refresh();
      onClose();
    },
  });

  const [name, setName] = useState(printer?.name ?? "");
  const [listen, setListen] = useState(printer?.listen ?? "");
  const [dialect, setDialect] = useState<Dialect>(printer?.dialect ?? "escpos");
  const [model, setModel] = useState(printer?.profile ?? "default");
  const chosen = models.find((item) => item.name === model);
  const failed = add.error ?? update.error;

  const submit = (event: FormEvent) => {
    event.preventDefault();
    if (editing) update.mutate({ path: { name: printer.name }, body: { profile: model } });
    else add.mutate({ body: { name: name || undefined, listen: listen || undefined, dialect, profile: model } });
  };

  return (
    <form onSubmit={submit} className="flex min-h-0 flex-1 flex-col">
      <SheetHeader>
        <SheetTitle>{editing ? printer.name : "Add printer"}</SheetTitle>
        <SheetDescription>
          {editing
            ? "The model applies to what is printed from now on. The name, the address and the language are set when the printer is made."
            : "A new port that a program can print to. The language is chosen per port, like the emulation setting of a real printer."}
        </SheetDescription>
      </SheetHeader>
      <SheetBody>
        <Field label="Name">
          <Input aria-label="Name" placeholder="printer" disabled={editing} value={name} onChange={(event) => setName(event.target.value)} />
        </Field>
        <Field label="Address">
          <Input
            aria-label="Address"
            placeholder="127.0.0.1:0, any free port"
            disabled={editing}
            value={listen}
            onChange={(event) => setListen(event.target.value)}
          />
        </Field>
        <Field label="Language">
          <Select
            items={Object.entries(DIALECTS).map(([value, label]) => ({ value, label }))}
            value={dialect}
            disabled={editing}
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
          <ModelSelect models={models} value={model} onChange={setModel} label="Model" />
          {chosen && <span className="text-[13px] text-muted-foreground">{paperText(chosen)}</span>}
        </Field>
        {editing && <PrinterStatusSettings printer={printer} />}
        {failed && (
          <Alert variant="destructive">
            <AlertTitle>{editing ? "It was not saved" : "It was not added"}</AlertTitle>
            <AlertDescription>{refused(failed)}</AlertDescription>
          </Alert>
        )}
      </SheetBody>
      <SheetFooter>
        {editing && (
          <Button type="button" variant="destructive" onClick={() => onRemove(printer)}>
            Remove
          </Button>
        )}
        <div className="flex-1" />
        <Button type="button" variant="ghost" onClick={onClose}>
          Cancel
        </Button>
        <Button type="submit" disabled={add.isPending || update.isPending}>
          {editing ? "Save" : "Add printer"}
        </Button>
      </SheetFooter>
    </form>
  );
}

/** Makes a printer or sets one up, in the same form, at the side of the table. It is made again each time it opens. */
export function PrinterMutateSheet({
  mode,
  printer,
  onClose,
  onRemove,
}: {
  mode: PrinterMutateMode | null;
  printer?: PrinterInfo;
  onClose: () => void;
  onRemove: (printer: PrinterInfo) => void;
}) {
  return (
    <Sheet open={mode !== null} onOpenChange={(open) => !open && onClose()}>
      <SheetContent className="sm:max-w-md">
        {mode && <MutateForm key={printer?.name ?? "new"} mode={mode} printer={printer} onClose={onClose} onRemove={onRemove} />}
      </SheetContent>
    </Sheet>
  );
}
