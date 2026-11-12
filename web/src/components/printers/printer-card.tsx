import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { listPrintersQueryKey, listProfilesOptions, updatePrinterMutation } from "@/api/generated/@tanstack/react-query.gen";
import type { PrinterInfo } from "@/api/generated/types.gen";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { DIALECTS, paperText } from "@/lib/profile";
import { ModelPicker } from "./model-picker";
import { PrinterConnect } from "./printer-connect";
import { Field, refused } from "./printer-fields";
import { PrinterRealismSettings } from "./printer-realism-settings";
import { PrinterStatusSettings } from "./printer-status-settings";

/** What one printer is and what it pretends to be going through. Each change applies at once. */
export function PrinterCard({ printer, onRemove }: { printer: PrinterInfo; onRemove: (printer: PrinterInfo) => void }) {
  const queryClient = useQueryClient();
  const models = useQuery({ ...listProfilesOptions(), staleTime: Number.POSITIVE_INFINITY }).data ?? [];
  const update = useMutation({
    ...updatePrinterMutation(),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: listPrintersQueryKey() }),
  });
  const model = models.find((item) => item.name === printer.profile);
  const paper = model ? paperText(model) : "";

  return (
    <Card>
      <CardHeader>
        <CardTitle>{printer.name}</CardTitle>
        <CardDescription className="font-mono text-sm">{printer.listen}</CardDescription>
      </CardHeader>
      <CardContent className="flex flex-col gap-5">
        <PrinterConnect printer={printer} />
        <Field label="Model">
          <ModelPicker
            models={models}
            value={printer.profile}
            dialect={printer.dialect}
            onChange={(name) => update.mutate({ path: { name: printer.name }, body: { profile: name } })}
          />
        </Field>
        <div className="grid grid-cols-2 gap-4 text-sm">
          <Field label="Language">
            <span className="text-muted-foreground">{DIALECTS[printer.dialect]}</span>
          </Field>
          {paper && (
            <Field label="Paper">
              <span className="text-muted-foreground">{paper}</span>
            </Field>
          )}
        </div>
        {update.isError && (
          <Alert variant="destructive">
            <AlertTitle>The model was not changed</AlertTitle>
            <AlertDescription>{refused(update.error)}</AlertDescription>
          </Alert>
        )}
        <PrinterStatusSettings printer={printer} />
        <PrinterRealismSettings key={printer.hold_offline} printer={printer} />
        <Button variant="destructive" onClick={() => onRemove(printer)}>
          Delete printer
        </Button>
      </CardContent>
    </Card>
  );
}
