import { useMutation, useQueryClient } from "@tanstack/react-query";
import { listPrintersQueryKey, removePrinterMutation } from "@/api/generated/@tanstack/react-query.gen";
import type { PrinterInfo } from "@/api/generated/types.gen";
import { ConfirmationDialog } from "@/components/ui/confirmation-dialog";

/** Asks before a printer is removed, and removes it when it is confirmed. */
export function PrinterRemoveDialog({ printer, onClose }: { printer?: PrinterInfo; onClose: () => void }) {
  const queryClient = useQueryClient();
  const remove = useMutation({
    ...removePrinterMutation(),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: listPrintersQueryKey() }),
  });

  return (
    <ConfirmationDialog
      open={printer !== undefined}
      onOpenChange={(open) => !open && onClose()}
      title={`Remove ${printer?.name ?? "the printer"}?`}
      description={
        printer && (
          <>
            Its port {printer.listen} closes and the programs connected to it are dropped. The receipts it printed are kept.
          </>
        )
      }
      confirmText="Remove"
      tone="destructive"
      onConfirm={async () => {
        if (printer) await remove.mutateAsync({ path: { name: printer.name } });
      }}
    />
  );
}
