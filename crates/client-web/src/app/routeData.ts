import * as api from '../api';
import type { AppRoute, AppState } from './types';
import { currentLogFilterRequest } from './activities';
import { clearBrowseSessions } from './browseSession';

type RouteApi = Pick<typeof api, 'getCapabilities' | 'getLibraries' | 'getMetadataProviders' | 'getSettings' | 'getSystemActivities' | 'getUsers' | 'getHome' | 'getItems' | 'getItem' | 'getItemMetadata' | 'getPlaybackDecision' | 'getPerson' | 'getItemCredits' | 'getLogs' | 'searchItems'>;

/** Retain restored browsing data when the server's null scope means all libraries. */
export function prepareBrowseRoute(state: AppState, route: AppRoute): void {
  if (route.page !== 'home' && route.page !== 'browse-detail') return;
  if (!state.home || (state.home.library_id ?? undefined) !== route.libraryId) {
    state.home = undefined;
    state.libraryItems = [];
    state.homePreviewItemId = undefined;
    state.homePreviewCollectionId = undefined;
  }
  state.libraryItemsLoading = !state.home;
}

/** Publish independent sections as they arrive, discarding replies from an abandoned route. */
export async function loadRouteData(
  state: AppState,
  route: AppRoute,
  render: () => void,
  isCurrent: () => boolean,
  source: RouteApi = api,
): Promise<void> {
  if (route.page !== 'item' || state.itemSources?.itemId !== route.itemId) state.itemSources = undefined;
  if (route.page !== 'item' || state.itemCredits?.itemId !== route.itemId) state.itemCredits = undefined;
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
        state.libraryItems = [];
        state.searchResults = [];
        state.selectedItem = undefined;
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
  const loadItemSupportingData = (item: api.MediaItemDetail): Promise<unknown[]> => Promise.all([
    source.getHome(item.library_id).then((value) => { if (isCurrent()) { state.home = value; render(); } }),
    source.getItems(item.library_id).then((value) => { if (isCurrent()) state.libraryItems = value; }).finally(() => {
      if (isCurrent()) { state.libraryItemsLoading = false; render(); }
    }),
  ]);

  const loadArtistCredits = async (itemId: number): Promise<void> => {
    state.itemCredits = { itemId, loading: true, credits: [] };
    render();
    try {
      const credits = await source.getItemCredits(itemId);
      if (isCurrent()) state.itemCredits = { itemId, loading: false, credits };
    } catch (error: unknown) {
      if (error instanceof api.ApiError && [401, 403, 404].includes(error.status)) throw error;
      if (isCurrent()) state.itemCredits = { itemId, loading: false, credits: [], error: error instanceof Error ? error.message : 'Request failed' };
    }
    if (isCurrent()) render();
  };

  load('server capabilities', source.getCapabilities(), (value) => { state.capabilities = value; });
  load('libraries', source.getLibraries(), (value) => { state.libraries = value; });
  load('metadata providers', source.getMetadataProviders(), (value) => { state.metadataProviders = value; });
  if (admin) load('background activity', source.getSystemActivities(), (value) => { state.systemActivities = value.activities; });
  // Settings supply per-library language preferences on item pages; other browsing pages do not need them.
  if (admin && (route.page === 'settings' || route.page === 'item')) {
    load('settings', source.getSettings(), (value) => { state.settingsResponse = value; });
  }
  if (route.page === 'settings' && route.section === 'users' && state.bootstrap?.current_user?.admin) {
    load('users', source.getUsers(), (value) => { state.users = value; });
  }

  if (route.page === 'home' || route.page === 'browse-detail') {
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
    state.itemMetadataLoading = true;
    load('item details', source.getItem(route.itemId).then(async (item) => {
      if (!isCurrent()) return item;
      state.selectedItem = item;
      state.libraryItemsLoading = true;
      render();
      const credits = item.item_type === 'artist' ? loadArtistCredits(item.id) : Promise.resolve();
      await Promise.all([loadItemSupportingData(item), credits]);
      return item;
    }), () => { /* The overview was published before its supporting sections. */ });
    load('linked metadata', source.getItemMetadata(route.itemId).finally(() => {
      if (isCurrent()) state.itemMetadataLoading = false;
    }), (value) => { state.selectedItemMetadata = value; });
    load('playback information', source.getPlaybackDecision(route.itemId), (value) => { state.selectedPlayback = value; });
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
