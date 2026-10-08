import { useMutation, useQuery } from "@tanstack/react-query";
import { useMemo, useState } from "react";
import { toast } from "sonner";
import {
  listPrintersOptions,
  listProfilesOptions,
  listSamplesOptions,
  printSampleMutation,
} from "@/api/generated/@tanstack/react-query.gen";
import type { PrinterInfo, SampleSummary, VariantInfo } from "@/api/generated/types.gen";
import { PageLayout } from "@/components/layout/page";
import { SplitLayout } from "@/components/layout/split-layout";
import { DataTable } from "@/components/ui/data-table";
import { SampleCard } from "./sample-card";
import { createSamplesColumns } from "./samples-columns";

/** The ready made receipts that real printing libraries wrote. The one that is chosen has its card at the side. */
export function SamplesPage({ onOpen }: { onOpen: (receipt: number) => void }) {
  const samples = useQuery(listSamplesOptions()).data ?? [];
  const printers = useQuery(listPrintersOptions()).data ?? [];
  const models = useQuery({ ...listProfilesOptions(), staleTime: Number.POSITIVE_INFINITY }).data ?? [];
  const print = useMutation({
    ...printSampleMutation(),
    onSuccess: (printed) => {
      const [receipt] = printed.receipts;
      toast.success(`Printed to ${printed.printer}`, {
        description: printed.receipts.length === 1 ? `Receipt #${receipt}.` : `${printed.receipts.length} receipts.`,
        action: receipt === undefined ? undefined : { label: "See it on the live paper", onClick: () => onOpen(receipt) },
      });
    },
    onError: (error) => toast.error("It was not printed", { description: (error as { error?: string }).error ?? "The server refused it." }),
  });
  const [chosen, setChosen] = useState<string>();

  const sample = samples.find((item) => item.id === chosen) ?? samples[0];
  const send = (to: PrinterInfo, of: SampleSummary, how: VariantInfo) =>
    print.mutate({ path: { id: of.id }, query: { printer: to.name, library: how.library, dialect: how.dialect } });
  const columns = useMemo(
    () =>
      createSamplesColumns({
        paperMm: (profile) => models.find((model) => model.name === profile)?.width_mm,
        printers,
        printing: print.isPending,
        onPrint: send,
      }),
    [models, printers, print.isPending],
  );

  return (
    <PageLayout width="wide">
      <SplitLayout
        list={
          <DataTable
            data={samples}
            columns={columns}
            getRowId={(item) => item.id}
            selected={sample?.id}
            onSelect={setChosen}
            empty="There are no samples in this build."
          />
        }
        detail={sample && <SampleCard key={sample.id} sample={sample} printers={printers} printing={print.isPending} onPrint={send} />}
      />
    </PageLayout>
  );
}
