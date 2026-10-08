import { useMutation, useQuery } from "@tanstack/react-query";
import { useState } from "react";
import {
  getSamplePictureOptions,
  listPrintersOptions,
  listProfilesOptions,
  listSamplesOptions,
  printSampleMutation,
} from "@/api/generated/@tanstack/react-query.gen";
import { getSamplePicture, getSampleText } from "@/api/generated/sdk.gen";
import type { PrinterInfo, SampleSummary, VariantInfo } from "@/api/generated/types.gen";
import { ModelStrip } from "@/components/model-strip";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { DropdownMenu, DropdownMenuContent, DropdownMenuItem, DropdownMenuTrigger } from "@/components/ui/dropdown-menu";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";
import { DIALECTS } from "@/lib/profile";
import { save } from "@/lib/save";
import { useObjectUrl } from "@/lib/use-object-url";

const DOWNLOADS = [
  {
    file: "png",
    label: "Picture (PNG)",
    get: (id: string, variant: VariantInfo) =>
      getSamplePicture({ path: { id }, query: { library: variant.library, dialect: variant.dialect }, throwOnError: true }),
  },
  {
    file: "txt",
    label: "Text",
    get: (id: string, variant: VariantInfo) =>
      getSampleText({ path: { id }, query: { library: variant.library, dialect: variant.dialect }, throwOnError: true }),
  },
];

const keyOf = (variant: VariantInfo) => `${variant.library}/${variant.dialect}`;
const libraries = (sample: SampleSummary) => [...new Set(sample.variants.map((variant) => variant.library))].join(", ");

/** A printer reads one command language, so a variant goes to a printer that is set to it. */
const reads = (printer: PrinterInfo, variant: VariantInfo) => printer.dialect === variant.dialect;

/** What a sample prints, drawn on the paper of the model it was written for. */
function Picture({ sample, variant }: { sample: SampleSummary; variant: VariantInfo }) {
  const picture = useQuery(
    getSamplePictureOptions({ path: { id: sample.id }, query: { library: variant.library, dialect: variant.dialect } }),
  );
  return <img src={useObjectUrl(picture.data)} alt={`${sample.title} on paper`} className="mx-auto max-w-full bg-white" />;
}

/** The ready made receipts that real printing libraries wrote, to look at and to print. */
export function SamplesPage({ onOpen }: { onOpen: (receipt: number) => void }) {
  const samples = useQuery(listSamplesOptions());
  const printers = useQuery(listPrintersOptions());
  const profiles = useQuery({ ...listProfilesOptions(), staleTime: Number.POSITIVE_INFINITY });
  const print = useMutation(printSampleMutation());
  const [chosen, setChosen] = useState<string>();
  const [variantKey, setVariantKey] = useState<string>();
  const [printerName, setPrinterName] = useState<string>();

  const list = samples.data ?? [];
  const sample = list.find((item) => item.id === chosen) ?? list[0];
  if (!sample) return <p className="text-sm text-muted-foreground">There are no samples in this build.</p>;

  const available = printers.data ?? [];
  const variant = sample.variants.find((item) => keyOf(item) === variantKey) ?? sample.variants[0];
  if (!variant) return <p className="text-sm text-muted-foreground">{sample.title} has no variant.</p>;
  const printer = available.find((item) => item.name === printerName) ?? available.find((item) => reads(item, variant)) ?? available[0];
  const fits = Boolean(printer && reads(printer, variant));
  const paper = (name: string) => profiles.data?.find((item) => item.name === name)?.width_mm;

  const select = (id: string) => {
    setChosen(id);
    setVariantKey(undefined);
    setPrinterName(undefined);
    print.reset();
  };
  const send = (to: PrinterInfo, of: SampleSummary, how: VariantInfo) =>
    print.mutate({ path: { id: of.id }, query: { printer: to.name, library: how.library, dialect: how.dialect } });
  const result = print.data;

  return (
    <div className="grid min-w-0 gap-4 xl:grid-cols-[minmax(0,1fr)_26rem]">
      <div className="flex min-w-0 flex-col gap-3">
        <p className="text-sm text-muted-foreground">
          Receipts written by real printing libraries. Print one to a printer, or download its bytes.
        </p>
        <div className="rounded-xl border">
          <Table>
            <TableHeader>
              <TableRow>
                <TableHead>Sample</TableHead>
                <TableHead>Made for</TableHead>
                <TableHead>Written by</TableHead>
                <TableHead />
              </TableRow>
            </TableHeader>
            <TableBody>
              {list.map((item) => {
                const first = item.variants[0];
                const target = first && available.find((candidate) => reads(candidate, first));
                return (
                  <TableRow key={item.id} data-state={item.id === sample.id ? "selected" : undefined} onClick={() => select(item.id)}>
                    <TableCell className="max-w-0 whitespace-normal">
                      <div className="font-medium">{item.title}</div>
                      <div className="truncate text-[13px] text-muted-foreground">{item.about}</div>
                    </TableCell>
                    <TableCell className="text-muted-foreground">
                      {item.profile}
                      {paper(item.profile) ? `, ${paper(item.profile)} mm` : ""}
                    </TableCell>
                    <TableCell className="text-[13px] text-muted-foreground">{libraries(item)}</TableCell>
                    <TableCell className="text-right">
                      <Button
                        variant="outline"
                        size="sm"
                        disabled={!target || print.isPending}
                        onClick={(event) => {
                          event.stopPropagation();
                          if (target && first) send(target, item, first);
                        }}
                      >
                        Print
                      </Button>
                    </TableCell>
                  </TableRow>
                );
              })}
            </TableBody>
          </Table>
        </div>
        {result && (
          <Alert>
            <AlertTitle>Printed to {result.printer}</AlertTitle>
            <AlertDescription>
              {result.receipts.length === 1 ? `Receipt #${result.receipts[0]}.` : `${result.receipts.length} receipts.`}{" "}
              {result.receipts[0] !== undefined && (
                <button type="button" className="underline underline-offset-4" onClick={() => onOpen(result.receipts[0] as number)}>
                  See it on the live paper
                </button>
              )}
            </AlertDescription>
          </Alert>
        )}
        {print.isError && (
          <Alert variant="destructive">
            <AlertTitle>It was not printed</AlertTitle>
            <AlertDescription>{(print.error as { error?: string }).error ?? "The server refused it."}</AlertDescription>
          </Alert>
        )}
      </div>

      <Card className="h-fit">
        <CardHeader>
          <CardTitle>{sample.title}</CardTitle>
          <CardDescription>{sample.about}</CardDescription>
        </CardHeader>
        <CardContent className="flex flex-col gap-4">
          <div className="max-h-96 overflow-auto rounded-lg bg-muted/40 p-4">
            <Picture sample={sample} variant={variant} />
          </div>
          <div className="flex flex-col gap-2">
            <span className="text-sm font-medium">Made for</span>
            <ModelStrip profile={sample.profile} />
          </div>
          <div className="flex flex-col gap-2">
            <span className="text-sm font-medium">Variant</span>
            <Select
              items={sample.variants.map((item) => ({ value: keyOf(item), label: `${item.library} ${item.version}, ${DIALECTS[item.dialect]}` }))}
              value={keyOf(variant)}
              onValueChange={(key) => setVariantKey(String(key))}
            >
              <SelectTrigger aria-label="Variant" className="w-full">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                {sample.variants.map((item) => (
                  <SelectItem key={keyOf(item)} value={keyOf(item)}>
                    {item.library} {item.version}, {DIALECTS[item.dialect]}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          </div>
          <div className="flex flex-col gap-2">
            <span className="text-sm font-medium">Print to</span>
            <Select
              items={available.map((item) => ({ value: item.name, label: `${item.name}, ${item.profile}, ${DIALECTS[item.dialect]}` }))}
              value={printer?.name ?? null}
              onValueChange={(name) => setPrinterName(String(name))}
            >
              <SelectTrigger aria-label="Print to" className="w-full">
                <SelectValue placeholder="No printer" />
              </SelectTrigger>
              <SelectContent>
                {available.map((item) => (
                  <SelectItem key={item.name} value={item.name}>
                    {item.name}, {item.profile}, {DIALECTS[item.dialect]}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
            {printer && !fits && (
              <p className="text-[13px] text-muted-foreground">
                {printer.name} reads {DIALECTS[printer.dialect]}, and this variant is {DIALECTS[variant.dialect]}. Print it to a printer
                set to {DIALECTS[variant.dialect]}.
              </p>
            )}
            {printer && fits && printer.profile !== sample.profile && (
              <p className="text-[13px] text-muted-foreground">
                {printer.name} is a {printer.profile}. The lines keep their length and its paper decides the rest.
              </p>
            )}
          </div>
          <div className="flex gap-2">
            <Button className="flex-1" disabled={!printer || !fits || print.isPending} onClick={() => printer && send(printer, sample, variant)}>
              {printer ? `Print to ${printer.name}` : "No printer"}
            </Button>
            <DropdownMenu>
              <DropdownMenuTrigger render={<Button variant="outline" />}>Download</DropdownMenuTrigger>
              <DropdownMenuContent align="end">
                {DOWNLOADS.map(({ file, label, get }) => (
                  <DropdownMenuItem
                    key={file}
                    onClick={async () =>
                      save((await get(sample.id, variant)).data, `${sample.id}.${variant.library}.${variant.dialect}.${file}`)
                    }
                  >
                    {label}
                  </DropdownMenuItem>
                ))}
              </DropdownMenuContent>
            </DropdownMenu>
          </div>
        </CardContent>
      </Card>
    </div>
  );
}
