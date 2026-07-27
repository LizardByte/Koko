// Vite config mirroring the vanilla client's serving behavior
// (../client-web/vite.config.ts: host 127.0.0.1, port 4173) so the PoC is a
// drop-in for the same dev workflow.
//
// Storybook (npm run storybook) has its own Vite config via .storybook/main.ts
// (viteFinal); this file is the app build only. The @storybook/addon-vitest
// integration was scaffolded but is intentionally not wired here yet — it
// brings a headless-playwright browser-test runner which is more than the
// current docs/visual-DX goal. Add it back when we want story interaction tests.
import { sveltekit } from '@sveltejs/kit/vite';
import { defineConfig } from 'vite';

export default defineConfig({
  plugins: [sveltekit()],
  server: {
    host: '127.0.0.1',
    port: 4173,
    // Proxy to the Rust server via a subpath. All requests to /proxy/* are
    // forwarded to KOKO_API_TARGET (e.g. https://127.0.0.1:9191), with the
    // /proxy prefix stripped. The client sets VITE_API_BASE_URL=/proxy so
    // all API calls go through this proxy — no CORS, no route conflicts.
    //
    // Usage: KOKO_API_TARGET=https://127.0.0.1:9191 npm run dev
    proxy: process.env.KOKO_API_TARGET
      ? {
          '/proxy': {
            target: process.env.KOKO_API_TARGET,
            changeOrigin: true,
            secure: false,
            rewrite: (path) => path.replace(/^\/proxy/, ''),
          },
        }
      : undefined,
  },
  preview: {
    host: '127.0.0.1',
    port: 4173,
  },
});
