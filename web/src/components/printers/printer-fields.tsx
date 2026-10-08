import type { ReactNode } from "react";

export const refused = (error: unknown) => (error as { error?: string } | null)?.error ?? "The server refused it.";

export function Field({ label, children }: { label: string; children: ReactNode }) {
  return (
    <div className="flex flex-col gap-2">
      <span className="text-sm font-medium">{label}</span>
      {children}
    </div>
  );
}
