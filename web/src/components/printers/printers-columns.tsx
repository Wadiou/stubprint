import type { ColumnDef } from "@tanstack/react-table";
import { MoreHorizontal, Pencil, Trash2 } from "lucide-react";
import type { PrinterInfo } from "@/api/generated/types.gen";
import { Button } from "@/components/ui/button";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { port, printerState, type Tone } from "@/lib/printer";
import { DIALECTS } from "@/lib/profile";

export type PrinterRow = PrinterInfo & { receipts: number };

const DOT: Record<Tone, string> = { ok: "bg-green-500", warn: "bg-amber-500", bad: "bg-red-500" };

export interface PrintersColumnsOptions {
  onEdit: (printer: PrinterRow) => void;
  onRemove: (printer: PrinterRow) => void;
}

export function createPrintersColumns({ onEdit, onRemove }: PrintersColumnsOptions): ColumnDef<PrinterRow>[] {
  return [
    { accessorKey: "name", header: "Name", cell: ({ row }) => <span className="font-medium">{row.original.name}</span> },
    {
      accessorKey: "dialect",
      header: "Language",
      cell: ({ row }) => <span className="text-muted-foreground">{DIALECTS[row.original.dialect]}</span>,
    },
    { accessorKey: "profile", header: "Model", cell: ({ row }) => <span className="text-muted-foreground">{row.original.profile}</span> },
    {
      id: "port",
      header: "Port",
      cell: ({ row }) => <span className="font-mono text-[13px] text-muted-foreground">{port(row.original)}</span>,
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
      accessorKey: "receipts",
      header: () => <span className="block text-right">Receipts</span>,
      cell: ({ row }) => <span className="block text-right text-muted-foreground">{row.original.receipts}</span>,
    },
    {
      id: "actions",
      header: () => <span className="sr-only">Actions</span>,
      cell: ({ row }) => (
        <DropdownMenu>
          <DropdownMenuTrigger
            render={<Button type="button" variant="ghost" size="icon" aria-label={`Actions for ${row.original.name}`} onClick={(event) => event.stopPropagation()} />}
          >
            <MoreHorizontal />
          </DropdownMenuTrigger>
          <DropdownMenuContent align="end">
            <DropdownMenuItem onClick={() => onEdit(row.original)}>
              <Pencil />
              Edit
            </DropdownMenuItem>
            <DropdownMenuItem variant="destructive" onClick={() => onRemove(row.original)}>
              <Trash2 />
              Remove
            </DropdownMenuItem>
          </DropdownMenuContent>
        </DropdownMenu>
      ),
    },
  ];
}
