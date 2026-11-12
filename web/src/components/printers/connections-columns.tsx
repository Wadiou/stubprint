import { Link } from "@tanstack/react-router";
import type { ColumnDef } from "@tanstack/react-table";
import type { Connection } from "@/api/generated/types.gen";
import { bytesText, durationText, isOpen, lasted, piecesText } from "@/lib/connection";

const right = (label: string) => () => <span className="block text-right">{label}</span>;
const muted = (text: string | number) => <span className="block text-right text-muted-foreground">{text}</span>;

/** A connection in one row: everything the server knows about it, so none has to be opened to be read. */
export const connectionsColumns: ColumnDef<Connection>[] = [
  { accessorKey: "id", header: "#", cell: ({ row }) => <span className="text-muted-foreground">#{row.original.id}</span> },
  {
    accessorKey: "peer",
    header: "From",
    cell: ({ row }) => <span className="font-mono text-[13px] text-muted-foreground">{row.original.peer}</span>,
  },
  {
    id: "opened",
    header: "Opened",
    cell: ({ row }) => <span className="text-muted-foreground">{new Date(row.original.opened_at).toLocaleTimeString()}</span>,
  },
  {
    id: "duration",
    header: right("Duration"),
    cell: ({ row }) => {
      const length = lasted(row.original);
      return muted(length === undefined ? "Open" : durationText(length));
    },
  },
  { id: "in", header: right("In"), cell: ({ row }) => muted(bytesText(row.original.bytes_in)) },
  { id: "out", header: right("Out"), cell: ({ row }) => muted(bytesText(row.original.bytes_out)) },
  {
    id: "pieces",
    header: right("Pieces"),
    // What the bytes came in, each with its time, is the hint of the number.
    cell: ({ row }) => (
      <span className="block text-right text-muted-foreground" title={piecesText(row.original.chunks)}>
        {row.original.chunks.length}
      </span>
    ),
  },
  {
    id: "receipts",
    header: "Receipts",
    cell: ({ row }) =>
      row.original.receipts.length === 0 ? (
        <span className="text-muted-foreground">None</span>
      ) : (
        <span className="flex flex-wrap gap-x-2">
          {row.original.receipts.map((id) => (
            <Link key={id} to="/" search={{ receipt: id }} className="underline underline-offset-2">
              #{id}
            </Link>
          ))}
        </span>
      ),
  },
  {
    id: "result",
    header: "Result",
    cell: ({ row }) => (
      <span className={isOpen(row.original) ? "font-medium" : "text-muted-foreground"}>{isOpen(row.original) ? "Open" : "Closed"}</span>
    ),
  },
];
