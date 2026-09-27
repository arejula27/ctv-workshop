import { svelte } from '@sveltejs/vite-plugin-svelte'
import { defineConfig } from 'vite'

// In dev, forward backend paths to the running stack (Caddy on :8080 by default).
const backend = process.env.BACKEND_URL ?? 'http://localhost:8080'

export default defineConfig({
  plugins: [svelte()],
  server: {
    proxy: {
      '/api': backend,
      '/esplora': backend,
      '/ws': { target: backend, ws: true },
    },
  },
})
