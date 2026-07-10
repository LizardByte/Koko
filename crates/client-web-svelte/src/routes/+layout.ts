// SPA mode: disable SSR globally. Required for adapter-static with a fallback
// page, and correct for Koko's deployment (static bundle served by the Rust
// server for remote browsers, or loaded inside a Tauri webview for desktop).
// SvelteKit's SSR/load-functions/endpoints add no value here.
export const ssr = false;
export const prerender = false;
export const trailingSlash = 'ignore';
