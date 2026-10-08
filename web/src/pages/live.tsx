import { useMutation, useQuery } from "@tanstack/react-query";
import { ChevronLeft, ChevronRight } from "lucide-react";
import { type ReactNode, useMemo, useState } from "react";
import {
  clearReceiptsMutation,
  getReceiptTextOptions,
  getTraceOptions,
  listPrintersOptions,
  listReceiptsOptions,
} from "@/api/generated/@tanstack/react-query.gen";
import type { ReceiptSummary } from "@/api/generated/types.gen";
import { CommandList, type Row } from "@/components/commands";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { Input } from "@/components/ui/input";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { Slider } from "@/components/ui/slider";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { describe, stateRows } from "@/lib/commands";
import { hexBytes, hexDump } from "@/lib/hex";
import { useReceiptBytes } from "@/lib/use-bytes";

const DOWNLOADS = [
  { path: "picture", file: "png", label: "Picture (PNG)" },
  { path: "text", file: "txt", label: "Text" },
  { path: "bytes", file: "bin", label: "Bytes" },
];

const time = (receipt: ReceiptSummary) => new Date(receipt.at).toLocaleTimeString();
const offset = (at: number) => `0x${at.toString(16).padStart(4, "0")}`;

function Empty({ address }: { address?: string }) {
  return (
    <Card className="mx-auto max-w-lg">
      <CardHeader>
        <CardTitle>No receipts yet</CardTitle>
        <CardDescription>Print to {address ?? "a printer"} from your app, or try a sample.</CardDescription>
      </CardHeader>
    </Card>
  );
}

/** One receipt: its paper on the left, up to the command chosen, and its commands on the right. */
function ReceiptView({ receipt, actions }: { receipt: ReceiptSummary; actions: ReactNode }) {
  const id = receipt.id;
  const trace = useQuery({ ...getTraceOptions({ path: { id } }), staleTime: Number.POSITIVE_INFINITY });
  const bytes = useReceiptBytes(id);
  const commands = trace.data ?? [];
  const total = commands.length;
  const [shown, setShown] = useState<number>();
  const upto = shown ?? total;
  const partial = upto < total;
  const query = partial ? { upto } : undefined;
  const text = useQuery(getReceiptTextOptions({ path: { id }, query }));
  const [filter, setFilter] = useState("");

  const rows: Row[] = useMemo(
    () =>
      commands.map((command) => ({
        index: command.index,
        ...describe(command.event),
        start: command.start,
        end: command.end,
        hex: bytes ? hexBytes(bytes.subarray(command.start, Math.min(command.end, command.start + 16))) : "",
      })),
    [commands, bytes],
  );
  const shownRows = rows.filter((row) => `${row.label} ${row.detail}`.toLowerCase().includes(filter.toLowerCase()));
  const chosen = upto > 0 ? rows[upto - 1] : undefined;
  const believed = upto > 0 ? commands[upto - 1]?.state : undefined;
  const until = chosen ? chosen.end : 0;

  return (
    <div className="grid gap-4 lg:grid-cols-[minmax(0,1fr)_28rem]">
      <Tabs defaultValue="picture" className="min-w-0">
        <div className="flex flex-wrap items-center gap-2">
          <TabsList>
            <TabsTrigger value="picture">Picture</TabsTrigger>
            <TabsTrigger value="text">Text</TabsTrigger>
            <TabsTrigger value="bytes">Bytes</TabsTrigger>
          </TabsList>
          <div className="flex-1" />
          {actions}
        </div>
        <Card className="mt-3 items-center bg-muted/40 py-6">
          <CardContent className="w-full overflow-auto">
            <TabsContent value="picture" className="flex justify-center">
              <img
                src={`/api/receipts/${id}/picture${partial ? `?upto=${upto}` : ""}`}
                alt={`Receipt ${id}`}
                className="max-w-full bg-white"
              />
            </TabsContent>
            <TabsContent value="text" className="flex justify-center">
              <pre className="font-mono text-sm">{text.data}</pre>
            </TabsContent>
            <TabsContent value="bytes">
              <pre className="overflow-x-auto font-mono text-xs">{bytes && hexDump(bytes.subarray(0, partial ? until : bytes.length))}</pre>
            </TabsContent>
          </CardContent>
        </Card>
        <div className="mt-3 flex items-center gap-3">
          <Button
            variant="outline"
            size="icon-sm"
            aria-label="Previous command"
            disabled={upto <= 0}
            onClick={() => setShown(upto - 1)}
          >
            <ChevronLeft />
          </Button>
          <Button
            variant="outline"
            size="icon-sm"
            aria-label="Next command"
            disabled={upto >= total}
            onClick={() => setShown(upto + 1)}
          >
            <ChevronRight />
          </Button>
          <Slider
            aria-label="Command"
            min={0}
            max={Math.max(total, 1)}
            value={upto}
            onValueChange={(value) => setShown(Array.isArray(value) ? value[0] : value)}
          />
          <span className="shrink-0 text-sm text-muted-foreground">
            Command {upto} of {total}
          </span>
        </div>
      </Tabs>

      <Tabs defaultValue="commands" className="min-w-0">
        <TabsList>
          <TabsTrigger value="commands">Commands</TabsTrigger>
          <TabsTrigger value="state">State</TabsTrigger>
        </TabsList>
        <TabsContent value="commands" className="flex flex-col gap-3">
          <Input aria-label="Filter commands" placeholder="Filter commands" value={filter} onChange={(e) => setFilter(e.target.value)} />
          <CommandList rows={shownRows} selected={upto - 1} onSelect={(index) => setShown(index + 1)} />
          {chosen && (
            <Card>
              <CardHeader>
                <CardTitle>{chosen.label}</CardTitle>
                <CardDescription>
                  offset {offset(chosen.start)}, {chosen.end - chosen.start} bytes
                </CardDescription>
              </CardHeader>
              <CardContent className="flex flex-col gap-2 text-sm">
                {chosen.detail && <div className="text-muted-foreground">{chosen.detail}</div>}
                <div className="rounded-md bg-muted px-2 py-1.5 font-mono text-xs break-all">
                  {bytes && hexBytes(bytes.subarray(chosen.start, chosen.end))}
                </div>
              </CardContent>
            </Card>
          )}
        </TabsContent>
        <TabsContent value="state">
          {believed ? (
            <div className="rounded-md border">
              {stateRows(believed).map((row) => (
                <div key={row.name} className="flex justify-between gap-3 border-b px-3 py-2 text-sm last:border-b-0">
                  <span className="text-muted-foreground">{row.name}</span>
                  <span className="font-mono text-xs">{row.value}</span>
                </div>
              ))}
            </div>
          ) : (
            <p className="text-sm text-muted-foreground">Nothing is printed before the first command.</p>
          )}
        </TabsContent>
      </Tabs>
    </div>
  );
}

/** The newest receipt unless another was picked. */
export function LivePage({ picked, onPick }: { picked?: number; onPick: (id: number) => void }) {
  const receipts = useQuery(listReceiptsOptions());
  const printers = useQuery(listPrintersOptions());
  const clear = useMutation(clearReceiptsMutation());
  const list = receipts.data ?? [];
  const current = list.find((receipt) => receipt.id === picked) ?? list.at(-1);
  const printer = printers.data?.[0];

  if (!current) return <Empty address={printer && `${printer.listen}, ${printer.dialect}`} />;

  const items = [...list].reverse().map((receipt) => ({
    value: String(receipt.id),
    label: `Receipt #${receipt.id}, ${receipt.printer}, ${time(receipt)}`,
  }));

  const actions = (
    <>
      <Select items={items} value={String(current.id)} onValueChange={(id) => onPick(Number(id))}>
        <SelectTrigger size="sm" aria-label="Receipt">
          <SelectValue />
        </SelectTrigger>
        <SelectContent>
          {items.map((item) => (
            <SelectItem key={item.value} value={item.value}>
              {item.label}
            </SelectItem>
          ))}
        </SelectContent>
      </Select>
      <DropdownMenu>
        <DropdownMenuTrigger render={<Button variant="outline" size="sm" />}>Download</DropdownMenuTrigger>
        <DropdownMenuContent align="end">
          {DOWNLOADS.map(({ path, file, label }) => (
            <DropdownMenuItem
              key={path}
              render={<a href={`/api/receipts/${current.id}/${path}`} download={`receipt-${current.id}.${file}`} />}
            >
              {label}
            </DropdownMenuItem>
          ))}
        </DropdownMenuContent>
      </DropdownMenu>
      <Button variant="ghost" size="sm" onClick={() => clear.mutate({})}>
        Clear
      </Button>
    </>
  );

  return <ReceiptView key={current.id} receipt={current} actions={actions} />;
}
