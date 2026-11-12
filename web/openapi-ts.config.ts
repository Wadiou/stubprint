import { defineConfig } from "@hey-api/openapi-ts";

/** Takes the `int64` format off every integer of the description, walking it as plain data. */
function dropInt64(node: unknown) {
  if (Array.isArray(node)) return node.forEach(dropInt64);
  if (typeof node !== "object" || node === null) return;
  // A nullable integer says its type as a list, such as `["integer", "null"]`.
  const integer = "type" in node && (node.type === "integer" || (Array.isArray(node.type) && node.type.includes("integer")));
  if (integer && "format" in node && node.format === "int64") delete node.format;
  Object.values(node).forEach(dropInt64);
}

export default defineConfig({
  input: "openapi.json",
  output: "src/api/generated",
  parser: {
    patch: {
      // The browser reads every number of the server as a number, and the ones the server sends
      // (counters, ids, times in milliseconds) stay far below 2^53. Left as `int64` the
      // schemas would ask for a `bigint`, which `JSON.parse` never gives.
      input: dropInt64,
    },
  },
  plugins: [
    { name: "@hey-api/client-fetch", runtimeConfigPath: "./src/api/runtime" },
    // Every request and every answer is checked against the schemas made from the server's
    // description, so what the page holds is what the server said it would send.
    { name: "@hey-api/sdk", validator: true },
    "zod",
    { name: "@tanstack/react-query", infiniteQueryOptions: false },
  ],
});
