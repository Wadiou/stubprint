import { useQuery } from "@tanstack/react-query";
import { listPrintersOptions } from "@/api/generated/@tanstack/react-query.gen";

/** The printers, which work all together: the live events keep this list as it is. */
export const usePrinters = () => useQuery(listPrintersOptions()).data ?? [];
