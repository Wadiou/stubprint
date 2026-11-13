import { useMutation, useQueryClient } from "@tanstack/react-query";
import { listPrintersQueryKey, updatePrinterMutation } from "@/api/generated/@tanstack/react-query.gen";
import type { PrinterInfo } from "@/api/generated/types.gen";
import {
  connectionLimit,
  DEFAULT_CONNECTION_LIMIT,
  DEFAULT_IDLE_TIMEOUT,
  idleTimeout,
  MAX_CONNECTION_LIMIT,
  MAX_IDLE_TIMEOUT,
} from "@/lib/printer";
import { NumberSetting, SettingRows } from "./number-setting";

/** How many programs the printer takes at once, and how long it waits for one that sends nothing. Off, it takes as many as it can and waits for ever. */
export function PrinterLimits({ printer }: { printer: PrinterInfo }) {
  const queryClient = useQueryClient();
  const update = useMutation({
    ...updatePrinterMutation(),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: listPrintersQueryKey() }),
  });
  const change = (body: { max_connections?: number; idle_timeout?: number }) => update.mutate({ path: { name: printer.name }, body });

  return (
    <section className="flex flex-col gap-3">
      <h3 className="font-semibold">Connections</h3>
      <SettingRows>
        <NumberSetting
          key={`limit-${printer.max_connections}`}
          label="Connection limit"
          description="Programs it takes at once. The next one is closed"
          unit="programs"
          value={printer.max_connections}
          fallback={DEFAULT_CONNECTION_LIMIT}
          parse={connectionLimit}
          error={`Use a whole number from 1 to ${MAX_CONNECTION_LIMIT}.`}
          apply={(value) => change({ max_connections: value })}
        />
        <NumberSetting
          key={`idle-${printer.idle_timeout}`}
          label="Idle timeout"
          description="Seconds a connection can send nothing before it is closed"
          unit="seconds"
          value={printer.idle_timeout}
          fallback={DEFAULT_IDLE_TIMEOUT}
          parse={idleTimeout}
          error={`Use a whole number of seconds from 1 to ${MAX_IDLE_TIMEOUT.toLocaleString("en-US")}.`}
          apply={(value) => change({ idle_timeout: value })}
        />
      </SettingRows>
    </section>
  );
}
