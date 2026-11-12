import type { PrinterInfo } from "@/api/generated/types.gen";
import { metresText } from "@/lib/printer";

/** What the printer counted since it was made. A program can ask for the line feeds and the cuts. */
export function PrinterUsage({ printer }: { printer: PrinterInfo }) {
  const { usage } = printer;
  const counters = [
    { label: "Receipts", value: usage.receipts.toLocaleString("en-US") },
    { label: "Cuts", value: usage.cuts.toLocaleString("en-US") },
    { label: "Line feeds", value: usage.line_feeds.toLocaleString("en-US") },
    { label: "Paper used", value: `${metresText(usage.paper_used)} m` },
  ];

  return (
    <section className="flex flex-col gap-3">
      <h3 className="font-semibold">Counters</h3>
      <dl className="grid grid-cols-2 gap-4 text-sm">
        {counters.map(({ label, value }) => (
          <div key={label} className="flex flex-col">
            <dt className="text-muted-foreground">{label}</dt>
            <dd className="font-medium">{value}</dd>
          </div>
        ))}
      </dl>
    </section>
  );
}
