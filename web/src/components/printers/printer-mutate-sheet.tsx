import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { type FormEvent, useState } from "react";
import { addPrinterMutation, listPrintersQueryKey, listProfilesOptions, updatePrinterMutation } from "@/api/generated/@tanstack/react-query.gen";
import type { Dialect, PrinterInfo } from "@/api/generated/types.gen";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Switch } from "@/components/ui/switch";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { Sheet, SheetBody, SheetContent, SheetFooter, SheetHeader, SheetTitle } from "@/components/ui/sheet";
import { useActivePrinter } from "@/lib/active-printer";
import { freeName } from "@/lib/printer";
import { DIALECTS } from "@/lib/profile";
import { Field, refused } from "./printer-fields";
import { ModelPicker } from "./model-picker";
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
  const { choose, printers } = useActivePrinter();
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

  const [name, setName] = useState(() => printer?.name ?? freeName(printers.map((item) => item.name)));
  const [anyPort, setAnyPort] = useState(true);
  const [listen, setListen] = useState(printer?.listen ?? "");
  const [dialect, setDialect] = useState<Dialect>(printer?.dialect ?? "escpos");
  const [model, setModel] = useState(printer?.profile ?? "default");
  const failed = add.error ?? update.error;

  const submit = (event: FormEvent) => {
    event.preventDefault();
    if (editing) update.mutate({ path: { name: printer.name }, body: { profile: model } });
    else add.mutate({ body: { name: name || undefined, listen: anyPort ? undefined : listen || undefined, dialect, profile: model } });
  };

  return (
    <form onSubmit={submit} className="flex min-h-0 flex-1 flex-col">
      <SheetHeader>
        <SheetTitle>{editing ? printer.name : "Add printer"}</SheetTitle>
      </SheetHeader>
      <SheetBody>
        <Field label="Name">
          <Input aria-label="Name" disabled={editing} value={name} onChange={(event) => setName(event.target.value)} />
        </Field>
        <Field label="Address">
          {editing ? (
            <Input aria-label="Address" disabled value={listen} />
          ) : (
            <>
              <div className="flex items-center gap-3">
                <Switch aria-label="Pick a free port" checked={anyPort} onCheckedChange={setAnyPort} />
                <span className="text-sm">Pick a free port</span>
              </div>
              <Input
                aria-label="Address"
                placeholder="127.0.0.1:9100"
                disabled={anyPort}
                value={anyPort ? "" : listen}
                onChange={(event) => setListen(event.target.value)}
              />
              {anyPort && <span className="text-[13px] text-muted-foreground">The system picks a port that is free, and it is shown in the table once the printer is added.</span>}
            </>
          )}
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
          <ModelPicker models={models} value={model} dialect={dialect} onChange={setModel} />
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
