// Vite config mirroring the vanilla client's serving behavior
// (../client-web/vite.config.ts: host 127.0.0.1, port 4173) so the PoC is a
// drop-in for the same dev workflow.
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
