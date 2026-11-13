import { readFileSync } from "node:fs";
import path from "node:path";
import tailwindcss from "@tailwindcss/vite";
import react from "@vitejs/plugin-react";
import { defineConfig } from "vite";

// The version of the program is the first version line of the Cargo.toml of the workspace, so the page
// never names one that was not released.
const cargo = readFileSync(new URL("../Cargo.toml", import.meta.url), "utf8");
const version = /^version = "(.+)"/m.exec(cargo)?.[1] ?? "0.0.0";

export default defineConfig({
  plugins: [react(), tailwindcss()],
  resolve: { alias: { "@": path.resolve(import.meta.dirname, "src") } },
  define: { __VERSION__: JSON.stringify(version) },
});
