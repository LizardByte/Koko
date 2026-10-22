/** Loads route-specific data while rejecting stale responses after navigation. */

import * as api from '../api';
import type { AppRoute, AppState } from './types';
import { currentLogFilterRequest } from './activities';
import { clearBrowseSessions } from './browseSession';

type RouteApi = Pick<typeof api, 'getCapabilities' | 'getLibraries' | 'getMetadataProviders' | 'getSettings' | 'getSystemActivities' | 'getUsers' | 'getHome' | 'getItems' | 'getItem' | 'getItemMetadata' | 'getPlaybackDecision' | 'getPerson' | 'getItemCredits' | 'getLogs' | 'searchItems'> & Partial<Pick<typeof api, 'getPlaylists' | 'getPlaylist' | 'getSimilarItems'>>;

/** Retain restored browsing data when the server's null scope means all libraries. */
export function prepareBrowseRoute(state: AppState, route: AppRoute): void {
  if (route.page !== 'home' && route.page !== 'browse-detail') return;
  if (!state.home || (state.home.library_id ?? undefined) !== route.libraryId) {
    state.home = undefined;
    state.playlists = undefined;
    state.selectedPlaylist = undefined;
    state.libraryItems = [];
    state.homePreviewItemId = undefined;
    state.homePreviewCollectionId = undefined;
  }
  state.libraryItemsLoading = !state.home;
}

/** Publish item pages together and other route sections independently, discarding stale replies. */
export async function loadRouteData(
  state: AppState,
  route: AppRoute,
  render: () => void,
  isCurrent: () => boolean,
  source: RouteApi = api,
): Promise<void> {
  clearStaleItemData(state, route);
  const admin = state.bootstrap?.current_user?.admin === true;
  if (!admin) {
    state.settingsResponse = undefined;
    state.users = [];
    state.systemActivities = [];
    state.logsResponse = undefined;
    state.dashboardItems = [];
    if (route.page === 'settings') return;
  }
  const tasks: Promise<void>[] = [];
  const load = <T>(label: string, request: Promise<T>, apply: (value: T) => void) => {
    tasks.push(request.then((value) => {
      if (!isCurrent()) return;
      apply(value);
      render();
    }).catch((error: unknown) => {
      if (!isCurrent()) return;
      if (error instanceof api.ApiError && [401, 403, 404].includes(error.status)) {
        clearBrowseSessions();
        state.home = undefined;
        state.playlists = undefined;
        state.selectedPlaylist = undefined;
        state.libraryItems = [];
        state.searchResults = [];
        state.selectedItem = undefined;
        state.similarItems = undefined;
        state.itemSources = undefined;
        state.itemCredits = undefined;
        state.selectedItemMetadata = undefined;
        state.selectedPlayback = undefined;
        state.selectedPerson = undefined;
        if (error.status === 401) {
          state.libraries = [];
          if (state.bootstrap) state.bootstrap = { ...state.bootstrap, current_user: undefined };
        }
        if (error.status === 401 || error.status === 403) {
          if (state.bootstrap?.current_user) state.bootstrap.current_user.admin = false;
          state.settingsResponse = undefined;
          state.users = [];
          state.logsResponse = undefined;
          state.systemActivities = [];
          state.dashboardItems = [];
        }
      }
      state.error = `Unable to load ${label}: ${error instanceof Error ? error.message : 'request failed'}`;
      render();
    }));
  };
  load('server capabilities', source.getCapabilities(), (value) => { state.capabilities = value; });
  load('libraries', source.getLibraries(), (value) => { state.libraries = value; });
  load('metadata providers', source.getMetadataProviders(), (value) => { state.metadataProviders = value; });
  if (admin) load('background activity', source.getSystemActivities(), (value) => { state.systemActivities = value.activities; });
  // Settings supply per-library language preferences on item pages; other browsing pages do not need them.
  if (needsRouteSettings(route, admin)) {
    load('settings', source.getSettings(), (value) => { state.settingsResponse = value; });
  }
  if (isUserSettingsRoute(route, admin)) {
    load('users', source.getUsers(), (value) => { state.users = value; });
  }

  if (route.page === 'home' || route.page === 'browse-detail') {
    loadPlaylistRoutes(state, route, source, load);
    load('featured media', source.getHome(route.libraryId), (value) => { state.home = value; });
    state.libraryItemsLoading = !state.home;
    load('library titles', source.getItems(route.libraryId).finally(() => {
      if (isCurrent()) state.libraryItemsLoading = false;
    }), (value) => { state.libraryItems = value; });
    if (state.searchQuery.trim()) {
      const query = state.searchQuery;
      load('search results', source.searchItems(query), (value) => {
        if (state.searchQuery === query) state.searchResults = value;
      });
    }
  } else if (route.page === 'item') {
    const userId = state.bootstrap?.current_user?.id;
    state.itemMetadataLoading = true;
    state.libraryItemsLoading = true;
    const knownLibraryId = state.libraryItems.find((item) => item.id === route.itemId)?.library_id;
    load('item details', getItemPageData(route.itemId, source, knownLibraryId).finally(() => {
      if (isCurrent()) {
        state.itemMetadataLoading = false;
        state.libraryItemsLoading = false;
      }
    }), (value) => {
      if (state.bootstrap?.current_user?.id !== userId) return;
      state.selectedItem = value.item;
      state.selectedItemMetadata = value.metadata;
      state.selectedPlayback = value.playback;
      state.home = value.home;
      state.libraryItems = value.libraryItems;
      state.itemCredits = value.itemCredits;
      state.similarItems = { itemId: route.itemId, userId, items: value.similarItems };
    });
  } else if (route.page === 'person') {
    load('person details', source.getPerson(route.personId), (value) => { state.selectedPerson = value; });
  } else if (route.section === 'dashboard') {
    load('metadata status', source.getItems(), (value) => { state.dashboardItems = value; });
  } else if (route.section === 'logs') {
    tasks.push(source.getLogs(currentLogFilterRequest()).then((value) => {
      if (isCurrent()) state.logsResponse = value;
    }).catch((error: unknown) => {
      if (isCurrent()) {
        state.logsResponse = undefined;
        state.logsError = error instanceof Error ? error.message : 'Failed to load logs.';
      }
    }).finally(() => {
      if (isCurrent()) { state.logsLoading = false; render(); }
    }));
  }
  await Promise.all(tasks);
}

/** Keep supporting rows during same-item revalidation, discarding recommendations from other accounts. */
function clearStaleItemData(state: AppState, route: AppRoute): void {
  if (route.page !== 'item' || state.itemSources?.itemId !== route.itemId) state.itemSources = undefined;
  if (route.page !== 'item' || state.itemCredits?.itemId !== route.itemId) state.itemCredits = undefined;
  if (route.page !== 'item' || state.similarItems?.itemId !== route.itemId
    || state.similarItems?.userId !== state.bootstrap?.current_user?.id) state.similarItems = undefined;
}

function needsRouteSettings(route: AppRoute, admin: boolean): boolean {
  return admin && (route.page === 'settings' || route.page === 'item');
}
type RouteLoader = <T>(label: string, request: Promise<T>, apply: (value: T) => void) => void;

/** Fetch saved item data concurrently, staging every row before publishing the page. */
async function getItemPageData(itemId: number, source: RouteApi, knownLibraryId?: number) {
  // Start collection and sibling data alongside item details when browsing already identified the
  // library. A stale library hint must not publish data or access errors for an item's new library.
  const prefetched = knownLibraryId === undefined ? undefined : getItemLibraryContext(knownLibraryId, source)
    .then(value => ({ value }), (error: unknown) => ({ error }));
  const details = source.getItem(itemId).then(async (item) => {
    const support = async () => {
      if (!prefetched || knownLibraryId !== item.library_id) return getItemLibraryContext(item.library_id, source);
      const result = await prefetched;
      if ('error' in result) throw result.error;
      return result.value;
    };
    const [context, itemCredits, similarItems] = await Promise.all([
      support(), getArtistCredits(item, source), getSimilarItems(item, source),
    ]);
    return { item, ...context, itemCredits, similarItems };
  });
  const [detail, metadata, playback] = await Promise.all([
    details, source.getItemMetadata(itemId), source.getPlaybackDecision(itemId),
  ]);
  return { ...detail, metadata, playback };
}

/** Fetch collection and sibling data together without waiting for the item's filesystem lookups. */
async function getItemLibraryContext(libraryId: number, source: RouteApi) {
  const [home, libraryItems] = await Promise.all([source.getHome(libraryId), source.getItems(libraryId)]);
  return { home, libraryItems };
}

/** Skip TV children and keep optional suggestion failures from preventing item navigation. */
async function getSimilarItems(item: api.MediaItemDetail, source: RouteApi): Promise<api.MediaItemSummary[]> {
  if (!api.supportsSimilarItems(item.item_type) || !source.getSimilarItems) return [];
  return source.getSimilarItems(item.id).catch((error: unknown) => {
    if (error instanceof api.ApiError && [401, 403, 404].includes(error.status)) throw error;
    return [];
  });
}

/** Preserve optional screen-credit errors while keeping access denials fatal to the item page. */
async function getArtistCredits(item: api.MediaItemDetail, source: RouteApi): Promise<AppState['itemCredits']> {
  if (item.item_type !== 'artist') return undefined;
  try {
    return { itemId: item.id, loading: false, credits: await source.getItemCredits(item.id) };
  } catch (error: unknown) {
    if (error instanceof api.ApiError && [401, 403, 404].includes(error.status)) throw error;
    return { itemId: item.id, loading: false, credits: [], error: error instanceof Error ? error.message : 'Request failed' };
  }
}

function loadPlaylistRoutes(state: AppState, route: AppRoute, source: RouteApi, load: RouteLoader): void {
  if (route.page !== 'home' && route.page !== 'browse-detail') return;
  if (source.getPlaylists) load('playlists', source.getPlaylists(route.libraryId), value => { state.playlists = value; });
  if (route.page === 'browse-detail' && route.kind === 'playlist' && source.getPlaylist && /^\d+$/.test(route.key)) {
    state.selectedPlaylist = undefined;
    load('playlist', source.getPlaylist(Number(route.key)), value => { state.selectedPlaylist = value; });
  }
}

function isUserSettingsRoute(route: AppRoute, admin: boolean): boolean {
  return admin && route.page === 'settings' && route.section === 'users';
}
