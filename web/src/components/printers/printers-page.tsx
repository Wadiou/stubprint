import { useQuery } from "@tanstack/react-query";
import { useMemo } from "react";
import { listConnectionsOptions, listReceiptsOptions } from "@/api/generated/@tanstack/react-query.gen";
import { PageLayout } from "@/components/layout/page";
import { SplitLayout } from "@/components/layout/split-layout";
import { Button } from "@/components/ui/button";
import { DataTable } from "@/components/ui/data-table";
import { isOpen } from "@/lib/connection";
import { usePrinters } from "@/lib/use-printers";
import { PrinterCard } from "./printer-card";
import { PrinterConnections } from "./printer-connections";
import { PrinterCreateSheet } from "./printer-create-sheet";
import { PrinterRemoveDialog } from "./printer-remove-dialog";
import { type PrinterRow, printersColumns } from "./printers-columns";

export interface PrintersSearch {
  /** The printer whose card is shown. The first one while none is named. */
  printer?: string;
  add?: boolean;
  remove?: string;
}

/** The printers and how each is doing. The one that is chosen has its card at the side. */
export function PrintersPage({ search, onSearchChange }: { search: PrintersSearch; onSearchChange: (next: PrintersSearch) => void }) {
  const printers = usePrinters();
  const receipts = useQuery(listReceiptsOptions()).data ?? [];

  const connections = useQuery(listConnectionsOptions()).data ?? [];

  const items = useMemo<PrinterRow[]>(
    () =>
      printers.map((printer) => ({
        ...printer,
        open: connections.filter((connection) => connection.printer === printer.name && isOpen(connection)).length,
        receipts: receipts.filter((receipt) => receipt.printer === printer.name).length,
      })),
    [printers, receipts, connections],
  );

  const chosen = printers.find((printer) => printer.name === search.printer) ?? printers[0];

  return (
    <PageLayout width="wide">
      <div className="flex justify-end">
        <Button onClick={() => onSearchChange({ ...search, add: true })}>Add printer</Button>
      </div>
      <SplitLayout
        list={
          <>
            <DataTable
              data={items}
              columns={printersColumns}
              getRowId={(printer) => printer.name}
              selected={chosen?.name}
              onSelect={(name) => onSearchChange({ printer: name })}
              empty="There is no printer. Add one to print to."
            />
            {chosen && <PrinterConnections key={chosen.name} printer={chosen} />}
          </>
        }
        detail={chosen && <PrinterCard key={chosen.name} printer={chosen} onRemove={(printer) => onSearchChange({ printer: chosen.name, remove: printer.name })} />}
      />
      <PrinterCreateSheet
        open={search.add === true}
        onAdded={(name) => onSearchChange({ printer: name })}
        onClose={() => onSearchChange({ printer: search.printer })}
      />
      <PrinterRemoveDialog
        printer={printers.find((printer) => printer.name === search.remove)}
        onClose={() => onSearchChange({ printer: search.printer })}
      />
    </PageLayout>
  );
}
