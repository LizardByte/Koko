/** Loads podcast season pages as the listener approaches the end, rejecting stale account replies. */
import { getPodcastSeasonEpisodes } from '../api';
import { state } from './state';

/** Bind progressive season loading with an accessible explicit fallback and abort-safe observers. */
export function bindPodcastPaging(signal: AbortSignal, context: { render: (preserveDom?: boolean) => void }, request = getPodcastSeasonEpisodes): void {
  const sentinel = document.querySelector<HTMLElement>('[data-podcast-more]');
  const item = state.selectedItem;
  const owner = state.bootstrap?.current_user?.id;
  if (!sentinel || !item?.podcast || item.item_type !== 'podcast_season' || owner === undefined) return;
  const current = () => state.bootstrap?.current_user?.id === owner && state.selectedItem === item;
  const load = async () => {
    if (!current() || item.children.length >= item.child_count) return;
    const active = state.podcastEpisodeLoading;
    if (active?.itemId === item.id && active.owner === owner && active.loading) return;
    const status = { itemId: item.id, owner, loading: true, error: undefined as string | undefined };
    state.podcastEpisodeLoading = status;
    const button = sentinel.querySelector<HTMLButtonElement>('button');
    if (button) { button.disabled = true; button.textContent = 'Loading episodes…'; }
    try {
      const page = await request(item.id, item.children.length);
      if (!current()) return;
      const known = new Set(item.children.map(child => child.id));
      item.children.push(...page.items.filter(child => !known.has(child.id)));
      item.child_count = page.total;
    } catch (error) {
      if (current()) status.error = error instanceof Error ? error.message : 'Episodes could not load.';
    } finally {
      status.loading = false;
      if (button) { button.disabled = false; button.textContent = status.error ? 'Retry loading episodes' : 'Load more episodes'; }
      if (current()) context.render(true);
    }
  };
  sentinel.querySelector('button')?.addEventListener('click', () => { void load(); }, { signal });
  if (typeof IntersectionObserver === 'undefined') return;
  const observer = new IntersectionObserver(entries => {
    const status = state.podcastEpisodeLoading;
    const failed = status?.itemId === item.id && status.owner === owner && status.error;
    if (entries.some(entry => entry.isIntersecting) && !failed) void load();
  }, { rootMargin: '600px' });
  observer.observe(sentinel);
  signal.addEventListener('abort', () => observer.disconnect(), { once: true });
}
