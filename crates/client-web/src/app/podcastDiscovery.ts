/** Searches the podcast directory without reloading the library or replacing newer account results. */
import { discoverPodcasts, getCountries } from '../api';
import type { AppState } from './types';
import { state } from './state';

interface DiscoveryContext {
  render: (preserveDom?: boolean) => void;
}

let pendingSearch: number | undefined;
let generation = 0;

/** Load reference data once per account view, preserving server labels and order. */
export async function loadPodcastCountries(target: AppState, isCurrent: () => boolean, render: () => void): Promise<void> {
  const data = target.podcasts;
  if (!data || data.tab !== 'discover' || data.countries) return;
  const current = () => isCurrent() && target.podcasts === data && target.bootstrap?.current_user?.id === data.owner;
  data.countriesError = undefined;
  try {
    const countries = await getCountries();
    if (current()) data.countries = countries;
  } catch (error) {
    if (current()) data.countriesError = error instanceof Error ? error.message : 'Country choices could not load.';
  } finally {
    if (current()) render();
  }
}

/** Invalidate queued searches and replies when leaving discovery or changing accounts. */
export function cancelPodcastDirectorySearch(): void {
  generation += 1;
  if (pendingSearch !== undefined) globalThis.clearTimeout(pendingSearch);
  pendingSearch = undefined;
}

/** Load one directory view, discarding replies for older queries, regions, routes, or accounts. */
export async function loadPodcastDirectory(target: AppState, isCurrent: () => boolean, render: () => void): Promise<void> {
  const data = target.podcasts;
  if (!data || data.tab !== 'discover') return;
  cancelPodcastDirectorySearch();
  const request = generation;
  const owner = data.owner, country = data.country, query = data.query;
  const current = () => request === generation && isCurrent() && target.podcasts === data
    && target.bootstrap?.current_user?.id === owner && data.tab === 'discover'
    && data.country === country && data.query === query;
  data.loading = true;
  data.error = undefined;
  render();
  try {
    const items = await discoverPodcasts(country, query.trim());
    if (current()) data.directory = items;
  } catch (error) {
    if (current()) {
      data.directory = [];
      data.error = error instanceof Error ? error.message : 'The podcast directory is unavailable. Add an RSS feed to subscribe.';
    }
  } finally {
    if (current()) { data.loading = false; render(); }
  }
}

/** Share the search/clear button style with global search; an empty query restores popular shows. */
export function bindPodcastDiscovery(signal: AbortSignal, context: DiscoveryContext): void {
  const form = document.querySelector<HTMLFormElement>('#podcast-search-form');
  const input = document.querySelector<HTMLInputElement>('#podcast-search');
  const country = document.querySelector<HTMLSelectElement>('#podcast-country');
  if (!form || !input || !country) return;
  const owner = state.bootstrap?.current_user?.id;
  const route = state.route;
  const current = () => state.route === route && state.bootstrap?.current_user?.id === owner
    && state.podcasts?.owner === owner && state.podcasts?.tab === 'discover';
  document.querySelector<HTMLButtonElement>('[data-podcast-countries-retry]')?.addEventListener('click', () => {
    if (current()) void loadPodcastCountries(state, current, () => context.render(true));
  }, { signal });
  const search = (): void => {
    if (!current()) return;
    void loadPodcastDirectory(state, current, () => context.render(true));
  };
  input.addEventListener('input', () => {
    if (!current() || !state.podcasts) return;
    cancelPodcastDirectorySearch();
    state.podcasts.query = input.value;
    state.podcasts.loading = true;
    pendingSearch = globalThis.setTimeout(search, 500);
    context.render(true);
  }, { signal });
  form.addEventListener('submit', event => {
    event.preventDefault();
    if (!current() || !state.podcasts) return;
    state.podcasts.query = input.value;
    search();
  }, { signal });
  country.addEventListener('change', () => {
    if (!current() || !state.podcasts) return;
    state.podcasts.country = country.value;
    search();
  }, { signal });
  document.querySelector<HTMLButtonElement>('[data-podcast-search-clear]')?.addEventListener('click', () => {
    if (!current() || !state.podcasts) return;
    state.podcasts.query = '';
    input.value = '';
    search();
    document.querySelector<HTMLInputElement>('#podcast-search')?.focus();
  }, { signal });
}
