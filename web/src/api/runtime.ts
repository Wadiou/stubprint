import type { CreateClientConfig } from "./generated/client.gen";

// The page and the API share one address, whatever port the program listens on.
export const createClientConfig: CreateClientConfig = (config) => ({
  ...config,
  baseUrl: window.location.origin,
});
