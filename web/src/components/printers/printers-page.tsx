import { useQuery } from "@tanstack/react-query";
import { useMemo } from "react";
import { listReceiptsOptions } from "@/api/generated/@tanstack/react-query.gen";
import { PageLayout } from "@/components/layout/page";
import { SplitLayout } from "@/components/layout/split-layout";
import { Button } from "@/components/ui/button";
import { DataTable } from "@/components/ui/data-table";
import { useActivePrinter } from "@/lib/active-printer";
import { PrinterCard } from "./printer-card";
import { PrinterCreateSheet } from "./printer-create-sheet";
import { PrinterRemoveDialog } from "./printer-remove-dialog";
import { type PrinterRow, printersColumns } from "./printers-columns";

export interface PrintersSearch {
  add?: boolean;
  remove?: string;
}

/** The printers and how each is doing. The one that is chosen has its card at the side. */
export function PrintersPage({ search, onSearchChange }: { search: PrintersSearch; onSearchChange: (next: PrintersSearch) => void }) {
  const { printer: active, printers, choose } = useActivePrinter();
  const receipts = useQuery(listReceiptsOptions()).data ?? [];

  const items = useMemo<PrinterRow[]>(
    () => printers.map((printer) => ({ ...printer, receipts: receipts.filter((receipt) => receipt.printer === printer.name).length })),
    [printers, receipts],
  );

  return (
    <PageLayout width="wide">
      <div className="flex justify-end">
        <Button onClick={() => onSearchChange({ add: true })}>Add printer</Button>
      </div>
      <SplitLayout
        list={
          <DataTable
            data={items}
            columns={printersColumns}
            getRowId={(printer) => printer.name}
            selected={active?.name}
            onSelect={choose}
            empty="There is no printer. Add one to print to."
          />
        }
        detail={active && <PrinterCard key={active.name} printer={active} onRemove={(printer) => onSearchChange({ remove: printer.name })} />}
      />
      <PrinterCreateSheet open={search.add === true} onClose={() => onSearchChange({})} />
      <PrinterRemoveDialog printer={printers.find((printer) => printer.name === search.remove)} onClose={() => onSearchChange({})} />
    </PageLayout>
  );
}
