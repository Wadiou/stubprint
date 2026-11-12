import { useMutation, useQueryClient } from "@tanstack/react-query";
import type { ReactNode } from "react";
import { listPrintersQueryKey, setPrinterStatusMutation } from "@/api/generated/@tanstack/react-query.gen";
import type { Fault, Paper, PrinterInfo } from "@/api/generated/types.gen";
import { zFault, zPaper } from "@/api/generated/zod.gen";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { Switch } from "@/components/ui/switch";

const PAPER: { value: Paper; label: string }[] = [
  { value: "adequate", label: "Adequate" },
  { value: "near_end", label: "Near end" },
  { value: "end", label: "Paper end" },
];
const FAULTS: { value: Fault | "none"; label: string }[] = [
  { value: "none", label: "None" },
  { value: "cutter", label: "Cutter" },
  { value: "unrecoverable", label: "Unrecoverable" },
  { value: "recoverable", label: "Recoverable" },
];

function Setting({ label, children }: { label: string; children: ReactNode }) {
  return (
    <div className="flex items-center gap-3 border-t py-3 first:border-t-0 first:pt-0">
      <span className="flex-1 font-medium">{label}</span>
      {children}
    </div>
  );
}

/** What the printer pretends to be going through. Each change applies at once. */
export function PrinterStatusSettings({ printer }: { printer: PrinterInfo }) {
  const queryClient = useQueryClient();
  const status = useMutation({
    ...setPrinterStatusMutation(),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: listPrintersQueryKey() }),
  });
  const set = (change: Parameters<typeof status.mutate>[0]["body"]) => status.mutate({ path: { name: printer.name }, body: change });
  const toggles = [
    { label: "Online", on: printer.status.online, change: (on: boolean) => set({ online: on }) },
    { label: "Cover open", on: printer.status.cover_open, change: (on: boolean) => set({ cover_open: on }) },
    { label: "Cash drawer open", on: printer.status.drawer_open, change: (on: boolean) => set({ drawer_open: on }) },
    { label: "Feed button pressed", on: printer.status.feed_button, change: (on: boolean) => set({ feed_button: on }) },
  ];

  return (
    <section className="flex flex-col">
      <h3 className="font-semibold">Status</h3>
      <p className="mt-1 mb-3 text-[13px] text-muted-foreground">What the printer reports. It applies at once to open connections and status commands.</p>
      {toggles.map(({ label, on, change }) => (
        <Setting key={label} label={label}>
          <Switch aria-label={label} checked={on} onCheckedChange={change} />
        </Setting>
      ))}
      <Setting label="Paper">
        <Select items={PAPER} value={printer.status.paper} onValueChange={(value) => {
            const paper = zPaper.safeParse(value);
            if (paper.success) set({ paper: paper.data });
          }}>
          <SelectTrigger aria-label="Paper" size="sm" className="w-40">
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            {PAPER.map((item) => (
              <SelectItem key={item.value} value={item.value}>
                {item.label}
              </SelectItem>
            ))}
          </SelectContent>
        </Select>
      </Setting>
      <Setting label="Fault">
        <Select
          items={FAULTS}
          value={printer.status.fault ?? "none"}
          onValueChange={(value) => {
            if (value === "none") return set({ fault: null });
            const fault = zFault.safeParse(value);
            if (fault.success) set({ fault: fault.data });
          }}
        >
          <SelectTrigger aria-label="Fault" size="sm" className="w-40">
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            {FAULTS.map((item) => (
              <SelectItem key={item.value} value={item.value}>
                {item.label}
              </SelectItem>
            ))}
          </SelectContent>
        </Select>
      </Setting>
    </section>
  );
}
