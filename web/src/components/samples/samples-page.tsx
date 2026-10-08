import { useMutation, useQuery } from "@tanstack/react-query";
import { useMemo, useState } from "react";
import {
  listPrintersOptions,
  listProfilesOptions,
  listSamplesOptions,
  printSampleMutation,
} from "@/api/generated/@tanstack/react-query.gen";
import type { PrinterInfo, SampleSummary, VariantInfo } from "@/api/generated/types.gen";
import { PageLayout } from "@/components/layout/page";
import { SplitLayout } from "@/components/layout/split-layout";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { DataTable } from "@/components/ui/data-table";
import { SampleCard } from "./sample-card";
import { createSamplesColumns } from "./samples-columns";

/** The ready made receipts that real printing libraries wrote. The one that is chosen has its card at the side. */
export function SamplesPage({ onOpen }: { onOpen: (receipt: number) => void }) {
  const samples = useQuery(listSamplesOptions()).data ?? [];
  const printers = useQuery(listPrintersOptions()).data ?? [];
  const models = useQuery({ ...listProfilesOptions(), staleTime: Number.POSITIVE_INFINITY }).data ?? [];
  const print = useMutation(printSampleMutation());
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
  const result = print.data;

  return (
    <PageLayout width="wide">
      <SplitLayout
        list={
          <>
            <DataTable
              data={samples}
              columns={columns}
              getRowId={(item) => item.id}
              selected={sample?.id}
              onSelect={(id) => {
                setChosen(id);
                print.reset();
              }}
              empty="There are no samples in this build."
            />
            {result && (
              <Alert>
                <AlertTitle>Printed to {result.printer}</AlertTitle>
                <AlertDescription>
                  {result.receipts.length === 1 ? `Receipt #${result.receipts[0]}.` : `${result.receipts.length} receipts.`}{" "}
                  {result.receipts[0] !== undefined && (
                    <button type="button" className="underline underline-offset-4" onClick={() => onOpen(result.receipts[0] as number)}>
                      See it on the live paper
                    </button>
                  )}
                </AlertDescription>
              </Alert>
            )}
            {print.isError && (
              <Alert variant="destructive">
                <AlertTitle>It was not printed</AlertTitle>
                <AlertDescription>{(print.error as { error?: string }).error ?? "The server refused it."}</AlertDescription>
              </Alert>
            )}
          </>
        }
        detail={sample && <SampleCard key={sample.id} sample={sample} printers={printers} printing={print.isPending} onPrint={send} />}
      />
    </PageLayout>
  );
}
