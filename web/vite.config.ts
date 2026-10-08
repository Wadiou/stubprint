import path from "node:path";
import tailwindcss from "@tailwindcss/vite";
import react from "@vitejs/plugin-react";
import { defineConfig } from "vitest/config";

// The program serves this build, so the page and the API share one address and need no CORS.
// Under `vite dev` the API is a stubprint on its default port, or where STUBPRINT_SERVER says.
export default defineConfig({
  plugins: [react(), tailwindcss()],
  resolve: { alias: { "@": path.resolve(import.meta.dirname, "src") } },
  server: { proxy: { "/api": process.env.STUBPRINT_SERVER ?? "http://127.0.0.1:8080" } },
  test: { globals: true },
});
