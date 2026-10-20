import { resolve } from "node:path"
import tailwindcss from "@tailwindcss/vite"
import react from "@vitejs/plugin-react"
import { defineConfig } from "vite"

// 127.0.0.1 rather than localhost: Node may resolve localhost to ::1, while
// the server binds 0.0.0.0 (IPv4).
const backend = process.env.BACKEND_URL ?? "http://127.0.0.1:8080"

// https://vite.dev/config/
export default defineConfig({
  plugins: [react(), tailwindcss()],
  resolve: {
    alias: {
      "@": resolve(import.meta.dirname, "./src"),
    },
  },
  build: {
    rolldownOptions: {
      output: {
        codeSplitting: {
          groups: [
            {
              name: "react",
              test: /node_modules[\\/](react|react-dom|react-router|scheduler)[\\/]/,
            },
            { name: "vendor", test: /node_modules/ },
          ],
        },
      },
    },
  },
  server: {
    // Spotify only accepts loopback redirect URIs by IP, so the dashboard (and
    // PUBLIC_URL) is http://127.0.0.1:5173 in development, never localhost.
    host: "127.0.0.1",
    port: 5173,
    strictPort: true,
    proxy: Object.fromEntries(
      ["/api", "/auth", "/oauth", "/.well-known", "/mcp"].map((path) => [
        path,
        backend,
      ])
    ),
  },
})
