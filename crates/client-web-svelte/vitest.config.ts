import { defineConfig } from 'vitest/config';
import { svelte } from '@sveltejs/vite-plugin-svelte';

export default defineConfig({
  plugins: [svelte({ hot: false })],
  test: {
    include: ['src/**/*.test.ts'],
    environment: 'node',
  },
  resolve: {
    conditions: ['browser'],
  },
});
