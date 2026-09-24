// Storybook stub for $app/state. The real SvelteKit module exports a reactive
// `page` populated by the router; in Storybook there's no router, so we provide
// a mutable runes-based mock that stories/decorators can set before render.
//
// Aliased via .storybook/main.ts vite resolve.alias: '$app/state' -> this file.
// Components read page.url.pathname / page.params.* reactively with $derived,
// so this must be a .svelte.js module (runes) for reactivity to work.

/**
 * The mock page state. Mutate `pathname` / `params` from a story or the
 * WithSvelteKit decorator to simulate a route.
 *
 *   import { setPage } from '$lib/storybook/mockAppState';
 *   setPage({ pathname: '/items/101', params: { id: '101' } });
 */
class MockPage {
  pathname = $state('/');
  params = $state<Record<string, string>>({});
  data = $state<Record<string, unknown>>({});
  error = $state<unknown>(null);
  route = $state<{ id: string }>({ id: '' });

  get url(): URL {
    // Reconstruct a URL object from the pathname (what components read).
    return new URL(this.pathname, 'http://localhost:6006');
  }
}

export const page = new MockPage();

export function setPage(next: {
  pathname?: string;
  params?: Record<string, string>;
  data?: Record<string, unknown>;
}): void {
  if (next.pathname !== undefined) page.pathname = next.pathname;
  if (next.params !== undefined) page.params = next.params;
  if (next.data !== undefined) page.data = next.data;
  page.error = null;
}

export const navigating = { from: null, to: null, type: null };
export const updated = {
  get current() {
    return false;
  },
  async check() {
    return false;
  },
};
