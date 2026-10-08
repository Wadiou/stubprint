import { flexRender, getCoreRowModel, useReactTable } from "@tanstack/react-table";
import { useMemo } from "react";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";
import { createPrintersColumns, type PrinterRow } from "./printers-columns";

export interface PrintersTableProps {
  items: PrinterRow[];
  active?: string;
  onChoose: (name: string) => void;
  onEdit: (printer: PrinterRow) => void;
  onRemove: (printer: PrinterRow) => void;
}

export function PrintersTable({ items, active, onChoose, onEdit, onRemove }: PrintersTableProps) {
  const columns = useMemo(() => createPrintersColumns({ onEdit, onRemove }), [onEdit, onRemove]);
  const table = useReactTable({ data: items, columns, getCoreRowModel: getCoreRowModel() });

  if (items.length === 0) {
    return <p className="rounded-lg border border-dashed px-4 py-12 text-center text-sm text-muted-foreground">There is no printer. Add one to print to.</p>;
  }

  return (
    <div className="rounded-lg border">
      <Table>
        <TableHeader>
          {table.getHeaderGroups().map((group) => (
            <TableRow key={group.id}>
              {group.headers.map((header) => (
                <TableHead key={header.id}>
                  {header.isPlaceholder ? null : flexRender(header.column.columnDef.header, header.getContext())}
                </TableHead>
              ))}
            </TableRow>
          ))}
        </TableHeader>
        <TableBody>
          {table.getRowModel().rows.map((row) => (
            <TableRow
              key={row.id}
              data-state={row.original.name === active ? "selected" : undefined}
              onClick={() => onChoose(row.original.name)}
            >
              {row.getVisibleCells().map((cell) => (
                <TableCell key={cell.id}>{flexRender(cell.column.columnDef.cell, cell.getContext())}</TableCell>
              ))}
            </TableRow>
          ))}
        </TableBody>
      </Table>
    </div>
  );
}
