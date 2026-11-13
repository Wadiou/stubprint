import { keepPreviousData, useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Link } from "@tanstack/react-router";
import { LayoutGrid, Printer, Receipt } from "lucide-react";
import { type ReactNode, useMemo, useState } from "react";
import {
  getReceiptPictureOptions,
  getReceiptTextOptions,
  getTraceOptions,
  listReceiptsOptions,
  listReceiptsQueryKey,
  removeReceiptMutation,
} from "@/api/generated/@tanstack/react-query.gen";
import { getReceiptBytes, getReceiptPicture, getReceiptText } from "@/api/generated/sdk.gen";
import type { ReceiptSummary } from "@/api/generated/types.gen";
import { CommandList, type Row } from "@/components/commands";
import { PageLayout } from "@/components/layout/page";
import { ModelStrip } from "@/components/model-strip";
import { ReceiptRail } from "@/components/receipt-rail";
import { ReceiptStateBadge } from "@/components/receipt-state";
import { StepSlider } from "@/components/step-slider";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { ConfirmationDialog } from "@/components/ui/confirmation-dialog";
import { Input } from "@/components/ui/input";
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

/** Nothing was printed yet: a way to try a sample at once, and to the printers. */
function Empty() {
  return (
    <PageLayout width="full">
      <div className="flex flex-col items-center gap-4 rounded-xl border border-dashed px-6 py-16 text-center">
        <div className="flex size-14 items-center justify-center rounded-xl bg-muted">
          <Receipt className="size-7 text-muted-foreground" />
        </div>
        <div className="flex flex-col gap-1">
          <h2 className="text-xl font-semibold tracking-tight">No receipts yet</h2>
          <p className="text-sm text-muted-foreground">They appear here as your app prints.</p>
        </div>
        <div className="flex flex-wrap justify-center gap-2">
          <Button render={<Link to="/samples" />}>
            <LayoutGrid />
            Try a sample
          </Button>
          <Button variant="outline" render={<Link to="/printers" />}>
            <Printer />
            Printers
          </Button>
        </div>
      </div>
    </PageLayout>
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
  // A receipt never changes, so each step is kept, and the one before stays in view while the next comes.
  const kept = { staleTime: Number.POSITIVE_INFINITY, placeholderData: keepPreviousData };
  const text = useQuery({ ...getReceiptTextOptions({ path: { id }, query }), ...kept });
  const picture = useQuery({ ...getReceiptPictureOptions({ path: { id }, query }), ...kept });
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
          <ModelStrip profile={receipt.profile} />
        </div>
        <div className="mt-3">
          <StepSlider value={upto} total={total} onChange={setShown} />
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
  const queryClient = useQueryClient();
  const remove = useMutation({
    ...removeReceiptMutation(),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: listReceiptsQueryKey() }),
  });
  const [removing, setRemoving] = useState(false);
  const list = receipts.data ?? [];
  const current = list.find((receipt) => receipt.id === picked) ?? list.at(-1);

  if (!current) return <Empty />;

  const actions = (
    <>
      <span className="text-sm text-muted-foreground">
        Receipt #{current.id}, {time(current)}, {current.printer}
      </span>
      <ReceiptStateBadge state={current.state} />
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
      <Button variant="ghost" size="sm" onClick={() => setRemoving(true)}>
        Delete
      </Button>
    </>
  );

  return (
    <PageLayout width="full">
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
      <ConfirmationDialog
        open={removing}
        onOpenChange={setRemoving}
        title={`Delete receipt #${current.id}?`}
        description="It is removed from the live paper and cannot be brought back."
        confirmText="Delete"
        tone="destructive"
        onConfirm={async () => {
          await remove.mutateAsync({ path: { id: current.id } });
          onPick(undefined);
        }}
      />
    </PageLayout>
  );
}
