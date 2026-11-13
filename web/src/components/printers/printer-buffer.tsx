import { useMutation, useQueryClient } from "@tanstack/react-query";
import { listPrintersQueryKey, updatePrinterMutation } from "@/api/generated/@tanstack/react-query.gen";
import type { PrinterInfo } from "@/api/generated/types.gen";
import { byteSize, DEFAULT_NETWORK_BUFFER, DEFAULT_PRINTER_BUFFER, MAX_BYTES } from "@/lib/printer";
import { NumberSetting, SettingRows } from "./number-setting";

const ERROR = `Use a whole number of bytes from 1 to ${MAX_BYTES.toLocaleString("en-US")}.`;

/** The buffers of the printer: what it holds while it cannot print and while it prints, and what the network keeps for a program. */
export function PrinterBuffer({ printer }: { printer: PrinterInfo }) {
  const queryClient = useQueryClient();
  const update = useMutation({
    ...updatePrinterMutation(),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: listPrintersQueryKey() }),
  });
  const change = (body: { printer_buffer?: number; network_buffer?: number }) => update.mutate({ path: { name: printer.name }, body });

  return (
    <section className="flex flex-col gap-3">
      <h3 className="font-semibold">Buffers</h3>
      <SettingRows>
        <NumberSetting
          key={`printer-${printer.printer_buffer}`}
          label="Printer buffer"
          description="Bytes kept while it cannot print, and while it prints"
          unit="bytes"
          value={printer.printer_buffer}
          fallback={DEFAULT_PRINTER_BUFFER}
          parse={byteSize}
          error={ERROR}
          apply={(bytes) => change({ printer_buffer: bytes })}
        />
        <NumberSetting
          key={`network-${printer.network_buffer}`}
          label="Network buffer"
          description="Bytes the network keeps for a program"
          unit="bytes"
          value={printer.network_buffer}
          fallback={DEFAULT_NETWORK_BUFFER}
          parse={byteSize}
          error={ERROR}
          apply={(bytes) => change({ network_buffer: bytes })}
        />
      </SettingRows>
    </section>
  );
}
