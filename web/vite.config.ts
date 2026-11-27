import { resolve } from "node:path"

import { backbone } from "@nat1anwastaken/mcp-backbone-dashboard/vite"
import { defineConfig } from "vite"

export default defineConfig({
  plugins: [
    backbone({ backend: process.env.BACKEND_URL ?? "http://127.0.0.1:8080" }),
  ],
  resolve: {
    alias: { "@": resolve(import.meta.dirname, "./src") },
  },
  // Spotify only accepts loopback IP redirect URIs, so the dev server must
  // be reached as 127.0.0.1, on a fixed port registered with the app.
  server: { host: "127.0.0.1", port: 5173, strictPort: true },
})
