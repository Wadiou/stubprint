import { cn } from "cn";
import type { ReactNode } from "react";

export type ContentWidth = "narrow" | "wide" | "full";

const WIDTHS: Record<ContentWidth, string> = {
  narrow: "max-w-4xl",
  wide: "max-w-7xl",
  full: "",
};

/** What every page sits in: centred, as wide as the page needs, with the same gap between its parts. */
export function PageLayout({ width, children }: { width: ContentWidth; children: ReactNode }) {
  return <div className={cn("mx-auto flex w-full min-w-0 flex-col gap-4", WIDTHS[width])}>{children}</div>;
}
