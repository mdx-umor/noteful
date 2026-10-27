import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

// Built straight into the folder the Vercel project serves.
export default defineConfig({
  plugins: [react()],
  build: { outDir: "../notebook-v1/web/public", emptyOutDir: true, target: "es2020" },
});
