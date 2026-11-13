import type { ColumnDef } from "@tanstack/react-table";
import { Loader2Icon } from "lucide-react";
import type { PrinterInfo, SampleSummary, VariantInfo } from "@/api/generated/types.gen";
import { Button } from "@/components/ui/button";

/** A printer reads one command language, so a variant goes to a printer that is set to it. */
export const reads = (printer: PrinterInfo, variant: VariantInfo) => printer.dialect === variant.dialect;

/** Where a sample goes when none is chosen: the printer of the model it was made for, else one that reads it. */
export const printerFor = (printers: PrinterInfo[], sample: SampleSummary, variant: VariantInfo) =>
  printers.find((printer) => printer.profile === sample.profile && reads(printer, variant)) ??
  printers.find((printer) => reads(printer, variant));

const libraries = (sample: SampleSummary) => [...new Set(sample.variants.map((variant) => variant.library))].join(", ");

export interface SamplesColumnsOptions {
  /** How wide the paper of a model is, in millimetres, when it says. */
  paperMm: (profile: string) => number | null | undefined;
  printers: PrinterInfo[];
  printing: boolean;
  onPrint: (printer: PrinterInfo, sample: SampleSummary, variant: VariantInfo) => void;
}

export function createSamplesColumns({ paperMm, printers, printing, onPrint }: SamplesColumnsOptions): ColumnDef<SampleSummary>[] {
  return [
    {
      accessorKey: "title",
      header: "Sample",
      cell: ({ row }) => <span className="font-medium">{row.original.title}</span>,
    },
    {
      accessorKey: "profile",
      header: "Made for",
      cell: ({ row }) => (
        <span className="text-muted-foreground">
          {row.original.profile}
          {paperMm(row.original.profile) ? `, ${paperMm(row.original.profile)} mm` : ""}
        </span>
      ),
    },
    {
      id: "libraries",
      header: "Written by",
      cell: ({ row }) => <span className="text-sm text-muted-foreground">{libraries(row.original)}</span>,
    },
    {
      id: "print",
      header: () => <span className="sr-only">Print</span>,
      cell: ({ row }) => {
        const first = row.original.variants[0];
        const target = first && printerFor(printers, row.original, first);
        return (
          <div className="text-right">
            <Button
              variant="outline"
              size="sm"
              disabled={!target || printing}
              onClick={(event) => {
                event.stopPropagation();
                if (target && first) onPrint(target, row.original, first);
              }}
            >
              {printing ? <Loader2Icon className="size-3.5 animate-spin" /> : "Print"}
            </Button>
          </div>
        );
      },
    },
  ];
}
