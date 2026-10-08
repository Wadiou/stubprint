import { useQuery } from "@tanstack/react-query";
import { listPrintersOptions, listReceiptsOptions } from "@/api/generated/@tanstack/react-query.gen";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";

export function LivePage() {
  const receipts = useQuery(listReceiptsOptions());
  const printers = useQuery(listPrintersOptions());
  const newest = receipts.data?.at(-1);
  const printer = printers.data?.[0];

  if (!newest) {
    return (
      <Card className="mx-auto max-w-lg">
        <CardHeader>
          <CardTitle>No receipts yet</CardTitle>
          <CardDescription>
            Print to {printer ? `${printer.listen}, ${printer.dialect}` : "a printer"} from your app,
            or try a sample.
          </CardDescription>
        </CardHeader>
      </Card>
    );
  }

  return (
    <div className="flex flex-col items-center gap-3">
      <div className="text-sm text-muted-foreground">
        Receipt #{newest.id}, {newest.printer}, {newest.bytes} bytes, {newest.events} commands
      </div>
      <img
        src={`/api/receipts/${newest.id}.png`}
        alt={`Receipt ${newest.id}`}
        className="max-w-full bg-white"
      />
    </div>
  );
}
