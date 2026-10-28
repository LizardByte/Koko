/** Loads podcast discovery and subscriptions and binds account-scoped podcast actions. */
import { addPodcast, discoverPodcasts, getPodcasts, getPodcastHome, refreshPodcast, removePodcast, setPodcastPlaybackOrder, type PodcastPlaybackOrder } from '../api';
import type { AppState } from './types';
import { state } from './state';
import { startPlaybackForItemId } from './playbackController';
import { stopMusic, toggleCurrentMusic, refreshPodcastQueue } from './audioPlayer';
import { activeMusicPlayback } from './musicPlaybackState';
import { bindPodcastPaging } from './podcastPaging';
import { clearBrowseSessions } from './browseSession';

/** Load independent catalog sections while ignoring replies from abandoned routes or accounts. */
export async function loadPodcasts(target: AppState, isCurrent: () => boolean, render: () => void): Promise<void> {
  const owner = target.bootstrap?.current_user?.id;
  if (owner === undefined) return;
  if (target.podcasts?.owner !== owner) target.podcasts = { owner, items: [], directory: [], country: 'us', query: '', loading: false };
  const data = target.podcasts!;
  const country = data.country, query = data.query, tab = data.tab ?? 'recommended';
  data.loading = data.tab === 'discover';
  data.error = undefined;
  const current = () => isCurrent() && target.bootstrap?.current_user?.id === owner && target.podcasts === data && data.country === country && data.query === query && (data.tab ?? 'recommended') === tab;
  await Promise.all([
    tab === 'recommended' ? getPodcastHome().then(home => { if (current()) { data.home = home; render(); } }).catch(error => { if (current()) data.error = error instanceof Error ? error.message : 'Podcast shelves could not load.'; }) : Promise.resolve(),
    getPodcasts().then(items => { if (current()) { data.items = items; render(); } }).catch(error => { if (current()) target.error = error instanceof Error ? error.message : 'Subscriptions could not load.'; }),
    data.tab === 'discover' ? discoverPodcasts(country, query).then(items => { if (current()) data.directory = items; }).catch(error => { if (current()) { data.directory = []; data.error = error instanceof Error ? error.message : 'The podcast directory is unavailable. Add a feed URL above.'; } }).finally(() => { if (current()) { data.loading = false; render(); } }) : Promise.resolve(),
  ]);
}

/** Bind buttons once per render; an account switch invalidates in-flight UI updates. */
export function bindPodcastActions(signal: AbortSignal, context: { navigateTo: (path: string) => void; refreshData: (showLoading?: boolean) => Promise<void>; render: (preserveDom?: boolean) => void }): void {
  document.querySelectorAll<HTMLElement>('[data-nav-podcasts]').forEach(button => button.addEventListener('click', () => context.navigateTo('/podcasts'), { signal }));
  const execute = async (work: () => Promise<void>, after?: () => void) => {
    const owner = state.bootstrap?.current_user?.id;
    if (state.podcasts?.busy) return;
    if (state.podcasts) state.podcasts.busy = true;
    context.render(true);
    try { await work(); if (state.bootstrap?.current_user?.id === owner) { after?.(); clearBrowseSessions(); await context.refreshData(false); } }
    catch (error) { if (state.bootstrap?.current_user?.id === owner) state.error = error instanceof Error ? error.message : 'Podcast action failed.'; }
    finally { if (state.bootstrap?.current_user?.id === owner) { if (state.podcasts) { state.podcasts.busy = false; }
      context.render(true); } }
  };
  document.querySelectorAll<HTMLButtonElement>('[data-podcast-tab]').forEach(button => button.addEventListener('click', () => {
    if (!state.podcasts) return;
    const tab = button.dataset.podcastTab;
    state.podcasts.tab = tab === 'discover' || tab === 'library' ? tab : 'recommended';
    state.searchQuery = ''; state.searchResults = []; state.showFullSearchResults = false;
    void context.refreshData(false);
  }, { signal }));
  document.querySelector<HTMLButtonElement>('[data-podcast-feed-toggle]')?.addEventListener('click', () => {
    if (!state.podcasts) return;
    state.podcasts.addFeedOpen = !state.podcasts.addFeedOpen;
    context.render(true);
  }, { signal });
  document.querySelectorAll<HTMLSelectElement>('[data-podcast-order]').forEach(select => select.addEventListener('change', () => {
    const order = select.value as PodcastPlaybackOrder;
    const id = Number(select.dataset.podcastOrder);
    void execute(async () => {
      await setPodcastPlaybackOrder(id, order);
      await refreshPodcastQueue(id);
    });
  }, { signal }));
  document.querySelector<HTMLFormElement>('#podcast-feed-form')?.addEventListener('submit', event => {
    event.preventDefault(); const url = String(new FormData(event.currentTarget as HTMLFormElement).get('feed') ?? '');
    void execute(async () => { await addPodcast(url); }, () => { if (state.podcasts) { state.podcasts.addFeedOpen = false; state.podcasts.tab = 'library'; } });
  }, { signal });
  document.querySelectorAll<HTMLButtonElement>('[data-podcast-add]').forEach(button => button.addEventListener('click', () => {
    const item = state.podcasts?.directory[Number(button.dataset.podcastAdd)];
    if (item) void execute(async () => { await addPodcast(item.feed_url); });
  }, { signal }));
  document.querySelectorAll<HTMLButtonElement>('[data-podcast-remove]').forEach(button => button.addEventListener('click', () => {
    const id = Number(button.dataset.podcastRemove);
    void execute(async () => {
      if (activeMusicPlayback()?.item.podcast?.podcast_id === id) stopMusic();
      await removePodcast(id);
    }, () => {
      if (activeMusicPlayback()?.item.podcast?.podcast_id === id) stopMusic();
      if (state.route.page === 'item') context.navigateTo('/podcasts');
    });
  }, { signal }));
  document.querySelectorAll<HTMLButtonElement>('[data-podcast-refresh]').forEach(button => button.addEventListener('click', () => {
    void execute(() => refreshPodcast(Number(button.dataset.podcastRefresh)));
  }, { signal }));
  document.querySelectorAll<HTMLButtonElement>('[data-podcast-play]').forEach(button => button.addEventListener('click', () => {
    const id = Number(button.dataset.podcastPlay);
    const play = async () => {
      if (await toggleCurrentMusic(id)) return;
      await startPlaybackForItemId(id, Number(button.dataset.podcastPosition ?? 0));
    };
    void play().catch(error => { state.error = error instanceof Error ? error.message : 'Episode could not start.'; context.render(true); });
  }, { signal }));
  bindPodcastPaging(signal, context);
  const search = (form: HTMLFormElement, popular = false) => {
    if (!state.podcasts) return;
    const fields = new FormData(form);
    state.podcasts.country = String(fields.get('country') ?? 'us').toLowerCase();
    state.podcasts.query = popular ? '' : String(fields.get('query') ?? '').trim();
    void context.refreshData(false);
  };
  const form = document.querySelector<HTMLFormElement>('#podcast-search-form');
  form?.addEventListener('submit', event => { event.preventDefault(); if (form.reportValidity()) search(form); }, { signal });
  document.querySelector<HTMLButtonElement>('[data-podcast-popular]')?.addEventListener('click', () => { if (form?.reportValidity()) search(form, true); }, { signal });
}
