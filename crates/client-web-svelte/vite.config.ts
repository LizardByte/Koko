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
    // Proxy API requests to the Rust server so the dev server can talk to
    // the real backend without CORS. Set KOKO_API_TARGET to the Rust server
    // URL (e.g. https://127.0.0.1:9191).
    //
    // /api/* is always proxied (no SvelteKit route conflict).
    // /login, /logout, /create_user are also SvelteKit page routes — but the
    // browser navigates to them via GET (SvelteKit), while the API calls are
    // POST. The proxy's bypass filter only proxies non-GET requests for those
    // paths, so SvelteKit handles page navigation and the proxy handles API calls.
    proxy: process.env.KOKO_API_TARGET
      ? {
          '/api': {
            target: process.env.KOKO_API_TARGET,
            changeOrigin: true,
            secure: false,
          },
          '/login': {
            target: process.env.KOKO_API_TARGET,
            changeOrigin: true,
            secure: false,
            // Only proxy POST (API call), let GET through to SvelteKit.
            bypass: (req) => (req.method === 'GET' ? req.url : undefined),
          },
          '/logout': {
            target: process.env.KOKO_API_TARGET,
            changeOrigin: true,
            secure: false,
            bypass: (req) => (req.method === 'GET' ? req.url : undefined),
          },
          '/create_user': {
            target: process.env.KOKO_API_TARGET,
            changeOrigin: true,
            secure: false,
            bypass: (req) => (req.method === 'GET' ? req.url : undefined),
          },
        }
      : undefined,
  },
  preview: {
    host: '127.0.0.1',
    port: 4173,
  },
});
