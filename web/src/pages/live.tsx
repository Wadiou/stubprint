import { useMutation, useQuery } from "@tanstack/react-query";
import { useEffect, useState } from "react";
import {
  clearReceiptsMutation,
  getReceiptBytesOptions,
  getReceiptTextOptions,
  listPrintersOptions,
  listReceiptsOptions,
} from "@/api/generated/@tanstack/react-query.gen";
import type { ReceiptSummary } from "@/api/generated/types.gen";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { hexDump } from "@/lib/hex";

const DOWNLOADS = [
  { path: "picture", file: "png", label: "Picture (PNG)" },
  { path: "text", file: "txt", label: "Text" },
  { path: "bytes", file: "bin", label: "Bytes" },
];

const time = (receipt: ReceiptSummary) => new Date(receipt.at).toLocaleTimeString();

function Picture({ id }: { id: number }) {
  return <img src={`/api/receipts/${id}/picture`} alt={`Receipt ${id}`} className="max-w-full bg-white" />;
}

function Text({ id }: { id: number }) {
  const text = useQuery(getReceiptTextOptions({ path: { id } }));
  return <pre className="font-mono text-sm">{text.data}</pre>;
}

function Bytes({ id }: { id: number }) {
  const file = useQuery(getReceiptBytesOptions({ path: { id } }));
  const [bytes, setBytes] = useState<Uint8Array>();
  useEffect(() => {
    let current = true;
    file.data?.arrayBuffer().then((buffer) => current && setBytes(new Uint8Array(buffer)));
    return () => {
      current = false;
    };
  }, [file.data]);
  return <pre className="overflow-x-auto font-mono text-xs">{bytes && hexDump(bytes)}</pre>;
}

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

/** The paper of one receipt, which is the newest unless another was picked. */
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

  return (
    <Tabs defaultValue="picture">
      <div className="flex flex-wrap items-center gap-2">
        <TabsList>
          <TabsTrigger value="picture">Picture</TabsTrigger>
          <TabsTrigger value="text">Text</TabsTrigger>
          <TabsTrigger value="bytes">Bytes</TabsTrigger>
        </TabsList>
        <div className="flex-1" />
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
      </div>
      <Card className="mt-3 items-center bg-muted/40 py-6">
        <CardContent className="w-full overflow-auto">
          <TabsContent value="picture" className="flex justify-center">
            <Picture id={current.id} />
          </TabsContent>
          <TabsContent value="text" className="flex justify-center">
            <Text id={current.id} />
          </TabsContent>
          <TabsContent value="bytes">
            <Bytes id={current.id} />
          </TabsContent>
        </CardContent>
      </Card>
    </Tabs>
  );
}
