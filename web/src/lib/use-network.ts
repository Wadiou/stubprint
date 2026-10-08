import { useQuery } from "@tanstack/react-query";
import { getNetworkOptions } from "@/api/generated/@tanstack/react-query.gen";

/** The addresses of the computer the server runs on,. */
export const useNetwork = () => useQuery(getNetworkOptions()).data;
