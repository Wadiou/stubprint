import { useQuery } from "@tanstack/react-query";
import { useMemo } from "react";
import { listConnectionsOptions } from "@/api/generated/@tanstack/react-query.gen";
import type { PrinterInfo } from "@/api/generated/types.gen";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { DataTable } from "@/components/ui/data-table";
import { isOpen } from "@/lib/connection";
import { connectionsColumns } from "./connections-columns";

/** The programs that connected to one printer: the open ones first, then the rest, each the newest first. */
export function PrinterConnections({ printer }: { printer: PrinterInfo }) {
  const connections = useQuery(listConnectionsOptions({ query: { printer: printer.name } })).data ?? [];

  const sorted = useMemo(
    () => [...connections].sort((a, b) => Number(isOpen(b)) - Number(isOpen(a)) || b.id - a.id),
    [connections],
  );

  return (
    <Card>
      <CardHeader>
        <CardTitle>Connections</CardTitle>
        <CardDescription>The programs that connected to {printer.name}. The server keeps the latest 200.</CardDescription>
      </CardHeader>
      <CardContent>
        <DataTable
          data={sorted}
          columns={connectionsColumns}
          getRowId={(connection) => String(connection.id)}
          empty={`Nothing has connected to ${printer.name} yet.`}
        />
      </CardContent>
    </Card>
  );
}
