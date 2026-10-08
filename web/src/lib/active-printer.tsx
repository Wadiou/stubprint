import { useQuery } from "@tanstack/react-query";
import { createContext, type ReactNode, useContext, useState } from "react";
import { listPrintersOptions } from "@/api/generated/@tanstack/react-query.gen";
import type { PrinterInfo } from "@/api/generated/types.gen";

const Chosen = createContext<{ name?: string; choose: (name: string) => void }>({ choose: () => {} });

/** Which printer the pages are about. It is the one that was chosen, or the first while none was. */
export function ActivePrinter({ children }: { children: ReactNode }) {
  const [name, choose] = useState<string>();
  return <Chosen.Provider value={{ name, choose }}>{children}</Chosen.Provider>;
}

export function useActivePrinter(): { printer?: PrinterInfo; printers: PrinterInfo[]; choose: (name: string) => void } {
  const printers = useQuery(listPrintersOptions()).data ?? [];
  const { name, choose } = useContext(Chosen);
  return { printer: printers.find((printer) => printer.name === name) ?? printers[0], printers, choose };
}
