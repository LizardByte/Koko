// SvelteKit config for the Koko Svelte PoC.
//
// Key architectural decision validated by this PoC: the app is a **static SPA**,
// NOT server-rendered. The Rust media server (see ../client-web) serves a static
// bundle over HTTP for remote browser clients (phones, tablets, LAN TVs), and a
// potential Tauri desktop shell would load the same built assets. SSR provides
// no value in either deployment, so we use adapter-static with a SPA fallback.
import adapter from '@sveltejs/adapter-static';
import { vitePreprocess } from '@sveltejs/vite-plugin-svelte';

/** @type {import('@sveltejs/kit').Config} */
const config = {
  preprocess: vitePreprocess(),
  kit: {
    adapter: adapter({
      // Output lands in `dist/` to match where the Rust server expects to find
      // the web client (see ../client-web: vite build -> dist).
      pages: 'dist',
      assets: 'dist',
      fallback: 'index.html',
      precompress: false,
      strict: false,
    }),
  },
};

export default config;
