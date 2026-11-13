import { useMutation, useQueryClient } from "@tanstack/react-query";
import { listPrintersQueryKey, updatePrinterMutation } from "@/api/generated/@tanstack/react-query.gen";
import type { PrinterInfo } from "@/api/generated/types.gen";
import { DEFAULT_SPEED, MAX_SPEED, printSpeed } from "@/lib/printer";
import { NumberSetting } from "./number-setting";

/** How fast the printer prints. A receipt keeps it busy while its paper comes out, and a program that sends more waits. Off, it takes no time. */
export function PrinterSpeed({ printer }: { printer: PrinterInfo }) {
  const queryClient = useQueryClient();
  const update = useMutation({
    ...updatePrinterMutation(),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: listPrintersQueryKey() }),
  });

  return (
    <section className="flex flex-col gap-3">
      <h3 className="font-semibold">Speed</h3>
      <NumberSetting
        label="Print speed"
        description="Each receipt keeps it busy while its paper comes out"
        unit="mm/s"
        value={printer.print_speed}
        fallback={DEFAULT_SPEED}
        parse={printSpeed}
        error={`Use a whole number of millimetres a second from 1 to ${MAX_SPEED}.`}
        inputLabel="Print speed in millimetres a second"
        apply={(speed) => update.mutate({ path: { name: printer.name }, body: { print_speed: speed } })}
      />
    </section>
  );
}
