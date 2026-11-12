import type { ReactNode } from "react";
import { zErrorBody } from "@/api/generated/zod.gen";

/** What the server said when it refused something, as its description of an error says it, or a general sentence. */
export function refused(error: unknown) {
  const body = zErrorBody.safeParse(error);
  return body.success ? body.data.error : "The server refused it.";
}

export function Field({ label, children }: { label: string; children: ReactNode }) {
  return (
    <div className="flex flex-col gap-2">
      <span className="text-sm font-medium">{label}</span>
      {children}
    </div>
  );
}
