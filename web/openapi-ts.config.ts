import { defineConfig } from "@hey-api/openapi-ts";

export default defineConfig({
  input: "openapi.json",
  output: "src/api/generated",
  plugins: [
    { name: "@hey-api/client-fetch", runtimeConfigPath: "./src/api/runtime" },
    "@hey-api/sdk",
    { name: "@tanstack/react-query", infiniteQueryOptions: false },
  ],
});
