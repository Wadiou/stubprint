import { useQuery } from "@tanstack/react-query";
import { getServerOptions } from "@/api/generated/@tanstack/react-query.gen";

/** How the server was started, and which version it is. */
export const useServer = () => useQuery(getServerOptions());
