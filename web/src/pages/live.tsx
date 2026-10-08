import { useMutation, useQuery } from "@tanstack/react-query";
import { ChevronLeft, ChevronRight } from "lucide-react";
import { type ReactNode, useMemo, useState } from "react";
import {
  clearReceiptsMutation,
  getReceiptPictureOptions,
  getReceiptTextOptions,
  getTraceOptions,
  listPrintersOptions,
  listReceiptsOptions,
} from "@/api/generated/@tanstack/react-query.gen";
import { getReceiptBytes, getReceiptPicture, getReceiptText } from "@/api/generated/sdk.gen";
import type { ReceiptSummary } from "@/api/generated/types.gen";
import { CommandList, type Row } from "@/components/commands";
import { ModelStrip } from "@/components/model-strip";
import { ReceiptRail } from "@/components/receipt-rail";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { Input } from "@/components/ui/input";
import { Slider } from "@/components/ui/slider";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { describe, stateRows } from "@/lib/commands";
import { hexBytes, hexDump } from "@/lib/hex";
import { save } from "@/lib/save";
import { useObjectUrl } from "@/lib/use-object-url";
import { useReceiptBytes } from "@/lib/use-bytes";

const DOWNLOADS = [
  { file: "png", label: "Picture (PNG)", get: (id: number) => getReceiptPicture({ path: { id }, throwOnError: true }) },
  { file: "txt", label: "Text", get: (id: number) => getReceiptText({ path: { id }, throwOnError: true }) },
  { file: "bin", label: "Bytes", get: (id: number) => getReceiptBytes({ path: { id }, throwOnError: true }) },
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
  const picture = useQuery(getReceiptPictureOptions({ path: { id }, query }));
  const paper = useObjectUrl(picture.data);
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
    <div className="grid min-w-0 gap-4 xl:grid-cols-[minmax(0,1fr)_24rem]">
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
        <div className="mt-3">
          <ModelStrip profile={receipt.profile} printer={receipt.printer} />
        </div>
        <Card className="mt-3 items-center bg-muted/40 py-6">
          <CardContent className="w-full overflow-auto">
            <TabsContent value="picture" className="flex justify-center">
              <img src={paper} alt={`Receipt ${id}`} className="max-w-full bg-white" />
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

/** The newest receipt while it follows them, or the one that was picked. */
export function LivePage({ picked, onPick }: { picked?: number; onPick: (id?: number) => void }) {
  const receipts = useQuery(listReceiptsOptions());
  const printers = useQuery(listPrintersOptions());
  const clear = useMutation(clearReceiptsMutation());
  const list = receipts.data ?? [];
  const current = list.find((receipt) => receipt.id === picked) ?? list.at(-1);
  const printer = printers.data?.[0];

  if (!current) return <Empty address={printer && `${printer.listen}, ${printer.dialect}`} />;

  const actions = (
    <>
      <span className="text-sm text-muted-foreground">
        Receipt #{current.id}, {time(current)}, {current.printer}
      </span>
      <DropdownMenu>
        <DropdownMenuTrigger render={<Button variant="outline" size="sm" />}>Download</DropdownMenuTrigger>
        <DropdownMenuContent align="end">
          {DOWNLOADS.map(({ file, label, get }) => (
            <DropdownMenuItem key={file} onClick={async () => save((await get(current.id)).data, `receipt-${current.id}.${file}`)}>
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

  return (
    <div className="grid min-w-0 gap-4 lg:grid-cols-[18rem_minmax(0,1fr)]">
      <ReceiptRail
        receipts={list}
        selected={current.id}
        following={picked === undefined}
        onPick={onPick}
        onFollow={(on) => onPick(on ? undefined : current.id)}
      />
      <ReceiptView key={current.id} receipt={current} actions={actions} />
    </div>
  );
}
