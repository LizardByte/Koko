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
  },
  preview: {
    host: '127.0.0.1',
    port: 4173,
  },
});
