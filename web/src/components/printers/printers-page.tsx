import { useQuery } from "@tanstack/react-query";
import { useCallback, useMemo } from "react";
import { listReceiptsOptions } from "@/api/generated/@tanstack/react-query.gen";
import { PageLayout } from "@/components/layout/page";
import { Button } from "@/components/ui/button";
import { useActivePrinter } from "@/lib/active-printer";
import { PrinterMutateSheet } from "./printer-mutate-sheet";
import { PrinterRemoveDialog } from "./printer-remove-dialog";
import type { PrinterRow } from "./printers-columns";
import { PrintersTable } from "./printers-table";

export interface PrintersSearch {
  add?: boolean;
  edit?: string;
  remove?: string;
}

/** The printers: which ones there are and how each is doing, to add, set up or remove. */
export function PrintersPage({ search, onSearchChange }: { search: PrintersSearch; onSearchChange: (next: PrintersSearch) => void }) {
  const { printer: active, printers, choose } = useActivePrinter();
  const receipts = useQuery(listReceiptsOptions()).data ?? [];

  const editing = printers.find((printer) => printer.name === search.edit);
  const mode = search.add ? "create" : editing ? "edit" : null;

  const items = useMemo<PrinterRow[]>(
    () => printers.map((printer) => ({ ...printer, receipts: receipts.filter((receipt) => receipt.printer === printer.name).length })),
    [printers, receipts],
  );
  const onEdit = useCallback((printer: PrinterRow) => onSearchChange({ edit: printer.name }), [onSearchChange]);
  const onRemove = useCallback((printer: { name: string }) => onSearchChange({ remove: printer.name }), [onSearchChange]);

  return (
    <PageLayout width="narrow">
      <div className="flex items-center gap-2">
        <p className="flex-1 text-sm text-muted-foreground">Each printer is a port that a program prints to.</p>
        <Button onClick={() => onSearchChange({ add: true })}>Add printer</Button>
      </div>
      <PrintersTable items={items} active={active?.name} onChoose={choose} onEdit={onEdit} onRemove={onRemove} />
      <PrinterMutateSheet mode={mode} printer={editing} onClose={() => onSearchChange({})} onRemove={onRemove} />
      <PrinterRemoveDialog printer={printers.find((printer) => printer.name === search.remove)} onClose={() => onSearchChange({})} />
    </PageLayout>
  );
}
