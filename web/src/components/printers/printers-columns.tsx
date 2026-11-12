import type { ColumnDef } from "@tanstack/react-table";
import type { PrinterInfo } from "@/api/generated/types.gen";
import { printerState, type Tone } from "@/lib/printer";
import { DIALECTS } from "@/lib/profile";

export type PrinterRow = PrinterInfo & { open: number; receipts: number };

const DOT: Record<Tone, string> = { ok: "bg-green-500", warn: "bg-amber-500", bad: "bg-red-500" };

export const printersColumns: ColumnDef<PrinterRow>[] = [
  { accessorKey: "name", header: "Name", cell: ({ row }) => <span className="font-medium">{row.original.name}</span> },
  {
    accessorKey: "dialect",
    header: "Language",
    cell: ({ row }) => <span className="text-muted-foreground">{DIALECTS[row.original.dialect]}</span>,
  },
  { accessorKey: "profile", header: "Model", cell: ({ row }) => <span className="text-muted-foreground">{row.original.profile}</span> },
  {
    id: "address",
    header: "Address",
    cell: ({ row }) => <span className="font-mono text-[13px] text-muted-foreground">{row.original.listen}</span>,
  },
  {
    id: "status",
    header: "Status",
    cell: ({ row }) => {
      const state = printerState(row.original.status);
      return (
        <span className="flex items-center gap-2">
          <span className={`size-2 rounded-full ${DOT[state.tone]}`} />
          {state.label}
        </span>
      );
    },
  },
  {
    accessorKey: "open",
    header: () => <span className="block text-right">Open</span>,
    cell: ({ row }) => <span className="block text-right text-muted-foreground">{row.original.open}</span>,
  },
  {
    accessorKey: "receipts",
    header: () => <span className="block text-right">Receipts</span>,
    cell: ({ row }) => <span className="block text-right text-muted-foreground">{row.original.receipts}</span>,
  },
];
