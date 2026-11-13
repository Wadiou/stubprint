import { type ColumnDef, flexRender, getCoreRowModel, useReactTable } from "@tanstack/react-table";
import type { ReactNode } from "react";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";

export interface DataTableProps<T> {
  data: T[];
  columns: ColumnDef<T>[];
  getRowId: (row: T) => string;
  /** The id of the row that is chosen. */
  selected?: string;
  onSelect?: (id: string) => void;
  /** What is said when there is no row and there is no `footer` either. */
  empty: string;
  /** A last row that is not one of `data`, such as a way to add one. Shown even with no rows. */
  footer?: ReactNode;
}

/** The table of every list: the rows it is given, one chosen, as the columns of its own file say. */
export function DataTable<T>({ data, columns, getRowId, selected, onSelect, empty, footer }: DataTableProps<T>) {
  const table = useReactTable({ data, columns, getRowId, getCoreRowModel: getCoreRowModel() });

  if (data.length === 0 && !footer) {
    return <p className="rounded-lg border border-dashed px-4 py-12 text-center text-sm text-muted-foreground">{empty}</p>;
  }

  return (
    <div className="rounded-lg border">
      <Table>
        {data.length > 0 && (
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
        )}
        <TableBody>
          {table.getRowModel().rows.map((row) => (
            <TableRow
              key={row.id}
              data-state={row.id === selected ? "selected" : undefined}
              // Only a table that can choose a row shows that its rows can be pressed.
              className={onSelect ? "cursor-pointer" : undefined}
              onClick={() => onSelect?.(row.id)}
            >
              {row.getVisibleCells().map((cell) => (
                <TableCell key={cell.id}>{flexRender(cell.column.columnDef.cell, cell.getContext())}</TableCell>
              ))}
            </TableRow>
          ))}
          {footer && (
            <TableRow>
              <TableCell colSpan={columns.length} className="p-0">
                {footer}
              </TableCell>
            </TableRow>
          )}
        </TableBody>
      </Table>
    </div>
  );
}
