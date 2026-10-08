import type { ReactNode } from "react";

/** A list with the card of the row that is chosen at its right, which is under it on a narrow screen. */
export function SplitLayout({ list, detail }: { list: ReactNode; detail?: ReactNode }) {
  return (
    <div className="grid min-w-0 items-start gap-4 xl:grid-cols-[minmax(0,1fr)_26rem]">
      <div className="flex min-w-0 flex-col gap-3">{list}</div>
      {detail && <div className="min-w-0 xl:sticky xl:top-6">{detail}</div>}
    </div>
  );
}
