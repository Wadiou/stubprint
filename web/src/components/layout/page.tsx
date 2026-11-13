import { cn } from "cn";
import type { ReactNode } from "react";

export type ContentWidth = "narrow" | "full";

const WIDTHS: Record<ContentWidth, string> = {
  narrow: "max-w-4xl",
  full: "",
};

/**
 * What every page sits in, with the same gap between its parts. A page of the dashboard fills the
 * width it has, and only a page that is a form, such as the settings, keeps to a narrow column.
 */
export function PageLayout({ width, children }: { width: ContentWidth; children: ReactNode }) {
  return <div className={cn("mx-auto flex w-full min-w-0 flex-1 flex-col gap-4", WIDTHS[width])}>{children}</div>;
}
