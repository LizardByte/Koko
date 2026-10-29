/** Loads podcast discovery and subscriptions and binds account-scoped podcast actions. */
import { addPodcast, getPodcasts, getPodcastHome, refreshPodcast, removePodcast, setPodcastPlaybackOrder, type PodcastPlaybackOrder } from '../api';
import type { AppState } from './types';
import { state } from './state';
import { startPlaybackForItemId } from './playbackController';
import { stopMusic, toggleCurrentMusic, refreshPodcastQueue } from './audioPlayer';
import { activeMusicPlayback } from './musicPlaybackState';
import { bindPodcastPaging } from './podcastPaging';
import { clearBrowseSessions } from './browseSession';
import { bindHoverPreview } from './hoverPreview';
import { bindPodcastDiscovery, cancelPodcastDirectorySearch, loadPodcastCountries, loadPodcastDirectory } from './podcastDiscovery';

/** Load independent catalog sections while ignoring replies from abandoned routes or accounts. */
export async function loadPodcasts(target: AppState, isCurrent: () => boolean, render: () => void): Promise<void> {
  const owner = target.bootstrap?.current_user?.id;
  if (owner === undefined || target.bootstrap?.podcasts_enabled === false) return;
  if (target.podcasts?.owner !== owner) target.podcasts = { owner, items: [], directory: [], country: 'us', query: '', loading: false };
  const data = target.podcasts!;
  const tab = data.tab ?? 'recommended';
  data.loading = data.tab === 'discover';
  data.error = undefined;
  const current = () => isCurrent() && target.bootstrap?.current_user?.id === owner && target.podcasts === data;
  await Promise.all([
    tab === 'recommended' ? getPodcastHome().then(home => { if (current()) { data.home = home; render(); } }).catch(error => { if (current()) data.error = error instanceof Error ? error.message : 'Podcast shelves could not load.'; }) : Promise.resolve(),
    getPodcasts().then(items => { if (current()) { data.items = items; render(); } }).catch(error => { if (current()) target.error = error instanceof Error ? error.message : 'Subscriptions could not load.'; }),
    loadPodcastDirectory(target, current, render),
    loadPodcastCountries(target, current, render),
  ]);
}

/** Bind buttons once per render; an account switch invalidates in-flight UI updates. */
export function bindPodcastActions(signal: AbortSignal, context: { navigateTo: (path: string) => void; refreshData: (showLoading?: boolean) => Promise<void>; render: (preserveDom?: boolean) => void }): void {
  document.querySelectorAll<HTMLElement>('[data-nav-podcasts]').forEach(button => button.addEventListener('click', () => context.navigateTo('/podcasts'), { signal }));
  const execute = (work: () => Promise<void>, after?: () => void) => executePodcastAction(context, work, after);
  document.querySelectorAll<HTMLButtonElement>('[data-podcast-tab]').forEach(button => button.addEventListener('click', () => {
    if (!state.podcasts) return;
    cancelPodcastDirectorySearch();
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
  bindPodcastAddActions(document, context, signal);
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
  bindPodcastDiscovery(signal, context);
}

interface PodcastActionContext {
  refreshData: (showLoading?: boolean) => Promise<void>;
  render: (preserveDom?: boolean) => void;
}

/** Use the shared hover delay and immediate focus preview for current directory cards only. */
export function bindPodcastDirectoryPreviews(root: ParentNode, refreshFeature: () => void): void {
  const owner = state.bootstrap?.current_user?.id;
  root.querySelectorAll<HTMLElement>('[data-podcast-preview-id]').forEach(element => {
    if (element.dataset.boundPodcastPreview === 'true') return;
    element.dataset.boundPodcastPreview = 'true';
    const update = (): void => {
      const data = state.podcasts;
      const id = element.dataset.podcastPreviewId;
      if (state.route.page !== 'podcasts' || data?.tab !== 'discover' || data.owner !== owner
        || state.bootstrap?.current_user?.id !== owner || data.previewDirectoryId === id
        || !data.directory.some(item => item.id === id)) return;
      data.previewDirectoryId = id;
      refreshFeature();
    };
    bindHoverPreview(element, update);
    element.addEventListener('focusin', update);
  });
}

/** Bind directory subscription buttons, including a hero replaced by a hover preview. */
export function bindPodcastAddActions(root: ParentNode, context: PodcastActionContext, signal?: AbortSignal): void {
  const owner = state.bootstrap?.current_user?.id;
  root.querySelectorAll<HTMLButtonElement>('[data-podcast-add]').forEach(button => button.addEventListener('click', () => {
    if (owner !== state.bootstrap?.current_user?.id || state.podcasts?.owner !== owner) return;
    const item = state.podcasts?.directory[Number(button.dataset.podcastAdd)];
    if (item) void executePodcastAction(context, async () => { await addPodcast(item.feed_url); });
  }, { signal }));
}

/** Preserve the failed operation so Retry repeats its request, retaining the original feed URL. */
export async function executePodcastAction(context: PodcastActionContext, work: () => Promise<void>, after?: () => void): Promise<void> {
  const owner = state.bootstrap?.current_user?.id;
  if (owner === undefined || state.podcasts?.busy) return;
  const route = state.route;
  const current = () => state.bootstrap?.current_user?.id === owner;
  if (state.podcasts) state.podcasts.busy = true;
  state.error = undefined;
  state.podcastRetry = undefined;
  context.render(true);
  try {
    await work();
    if (current()) { after?.(); clearBrowseSessions(); await context.refreshData(false); }
  } catch (error) {
    if (current()) {
      const message = error instanceof Error ? error.message : 'Podcast action failed.';
      state.error = message;
      state.podcastRetry = { owner, message, route, run: () => executePodcastAction(context, work, after) };
    }
  } finally {
    if (current()) { if (state.podcasts) { state.podcasts.busy = false; } context.render(true); }
  }
}

/** Do not repeat a different account's request or an operation unrelated to the current error. */
export async function retryPodcastAction(): Promise<boolean> {
  const retry = state.podcastRetry;
  if (!retry || retry.owner !== state.bootstrap?.current_user?.id || retry.message !== state.error || retry.route !== state.route) return false;
  await retry.run();
  return true;
}
