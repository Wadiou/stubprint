import type { ReactNode } from "react";

/**
 * A list with the card of the row that is chosen at its right, which is under it on a narrow screen.
 * The card never grows past the screen: it scrolls inside itself once it is taller, at every width,
 * and on a wide screen it also stays in view under the header while the list scrolls past it.
 */
export function SplitLayout({ list, detail }: { list: ReactNode; detail?: ReactNode }) {
  return (
    <div className="grid min-w-0 items-start gap-4 xl:grid-cols-[minmax(0,1fr)_26rem]">
      <div className="flex min-w-0 flex-col gap-3">{list}</div>
      {detail && (
        <div className="max-h-[calc(100dvh-6.5rem)] min-w-0 overflow-y-auto overscroll-contain scrollbar-thin xl:sticky xl:top-20">
          {detail}
        </div>
      )}
    </div>
  );
}
