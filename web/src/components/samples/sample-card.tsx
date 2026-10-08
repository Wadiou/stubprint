import { useQuery } from "@tanstack/react-query";
import { useState } from "react";
import { getSamplePictureOptions } from "@/api/generated/@tanstack/react-query.gen";
import { getSamplePicture, getSampleText } from "@/api/generated/sdk.gen";
import type { PrinterInfo, SampleSummary, VariantInfo } from "@/api/generated/types.gen";
import { PrinterCreateSheet } from "@/components/printers/printer-create-sheet";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { DropdownMenu, DropdownMenuContent, DropdownMenuItem, DropdownMenuTrigger } from "@/components/ui/dropdown-menu";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { DIALECTS } from "@/lib/profile";
import { save } from "@/lib/save";
import { useObjectUrl } from "@/lib/use-object-url";
import { printerFor, reads } from "./samples-columns";

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

/** What a sample prints, drawn on the paper of the model it was written for. */
function Picture({ sample, variant }: { sample: SampleSummary; variant: VariantInfo }) {
  const picture = useQuery(
    getSamplePictureOptions({ path: { id: sample.id }, query: { library: variant.library, dialect: variant.dialect } }),
  );
  return <img src={useObjectUrl(picture.data)} alt={`${sample.title} on paper`} className="mx-auto max-w-full bg-white" />;
}

/** One sample: how it looks, the variant and the printer to choose, to print or to download. */
export function SampleCard({
  sample,
  printers,
  printing,
  onPrint,
}: {
  sample: SampleSummary;
  printers: PrinterInfo[];
  printing: boolean;
  onPrint: (printer: PrinterInfo, sample: SampleSummary, variant: VariantInfo) => void;
}) {
  const [variantKey, setVariantKey] = useState<string>();
  const [printerName, setPrinterName] = useState<string>();
  const [adding, setAdding] = useState(false);

  const variant = sample.variants.find((item) => keyOf(item) === variantKey) ?? sample.variants[0];
  if (!variant) return null;
  const printer = printers.find((item) => item.name === printerName) ?? printerFor(printers, sample, variant) ?? printers[0];
  const fits = Boolean(printer && reads(printer, variant));

  return (
    <Card>
      <CardHeader>
        <CardTitle>{sample.title}</CardTitle>
        <CardDescription>{sample.about}</CardDescription>
      </CardHeader>
      <CardContent className="flex flex-col gap-4">
        <div className="max-h-96 overflow-auto rounded-lg bg-muted/40 p-4">
          <Picture sample={sample} variant={variant} />
        </div>
        <div className="flex items-center gap-3">
          <div className="min-w-0 flex-1">
            <div className="text-sm font-medium">Made for</div>
            <div className="truncate text-sm text-muted-foreground">{sample.profile}</div>
          </div>
          <Button variant="outline" size="sm" onClick={() => setAdding(true)}>
            Add this printer
          </Button>
        </div>
        <PrinterCreateSheet
          open={adding}
          preset={{ model: sample.profile, dialect: variant.dialect }}
          onClose={() => setAdding(false)}
        />
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
            items={printers.map((item) => ({ value: item.name, label: `${item.name}, ${item.profile}, ${DIALECTS[item.dialect]}` }))}
            value={printer?.name ?? null}
            onValueChange={(name) => setPrinterName(String(name))}
          >
            <SelectTrigger aria-label="Print to" className="w-full">
              <SelectValue placeholder="No printer" />
            </SelectTrigger>
            <SelectContent>
              {printers.map((item) => (
                <SelectItem key={item.name} value={item.name}>
                  {item.name}, {item.profile}, {DIALECTS[item.dialect]}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
          {printer && !fits && (
            <p className="text-[13px] text-muted-foreground">
              {printer.name} reads {DIALECTS[printer.dialect]}, and this variant is {DIALECTS[variant.dialect]}. Print it to a printer set to{" "}
              {DIALECTS[variant.dialect]}.
            </p>
          )}
        </div>
        <div className="flex gap-2">
          <Button className="flex-1" disabled={!printer || !fits || printing} onClick={() => printer && onPrint(printer, sample, variant)}>
            {printer ? `Print to ${printer.name}` : "No printer"}
          </Button>
          <DropdownMenu>
            <DropdownMenuTrigger render={<Button variant="outline" />}>Download</DropdownMenuTrigger>
            <DropdownMenuContent align="end">
              {DOWNLOADS.map(({ file, label, get }) => (
                <DropdownMenuItem
                  key={file}
                  onClick={async () => save((await get(sample.id, variant)).data, `${sample.id}.${variant.library}.${variant.dialect}.${file}`)}
                >
                  {label}
                </DropdownMenuItem>
              ))}
            </DropdownMenuContent>
          </DropdownMenu>
        </div>
      </CardContent>
    </Card>
  );
}
