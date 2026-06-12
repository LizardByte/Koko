/** Coordinates app startup, route-level data loading, and shell rendering. */
import kokoLogoUrl from '../../../assets/Koko.svg';
import { createIcons, icons } from 'lucide';
import {
  activeLibraryScanActivities,
  activeLibraryPendingRefreshCount,
  activeMetadataRefreshActivities,
  currentLogFilterRequest,
  itemIsMetadataPending,
  libraryRefreshProgress,
  snapshotJson,
} from './app/activities';
import {
  canManageUsers,
  currentUser,
  renderLoginScreen,
  renderAuthShell,
  renderWelcomeScreen,
  requiresLogin,
  requiresSetup,
} from './app/auth';
import { setElementHtml as patchElementHtml } from './app/domPatcher';
import { abortRenderEvents, bindEvents, type AppEventBindingContext } from './app/eventBindings';
import { escapeHtml } from './app/format';
import { bindGlobalInputHandlers } from './app/input';
import {
  configurePlaybackController,
  renderPlayerOverlay,
  syncThemeSongPlayer,
} from './app/playbackController';
import { defaultHomeTab, parseRoute } from './app/routes';
import {
  activeLibraryId,
  homeFeaturePreview,
  pageBackdropUrlForHomePreview,
  searchResultItems,
} from './app/selectors';
import { syncVisibleSpinners } from './app/spinners';
import { state } from './app/state';
import {
  renderHomePage,
  renderItemCard,
} from './app/homeView';
import {
  renderItemPage,
  renderPersonPage,
} from './app/itemPersonView';
import { renderSettingsPage } from './app/settingsView';
import {
  renderIcon,
  renderUserAvatar,
  selectedLibraryIcon,
} from './app/ui';
import {
  clearStoredAuthToken,
  getAppBootstrap,
  getApiMode,
  getArtworkUrl,
  getCapabilities,
  getHome,
  getItem,
  getItemMetadata,
  getItems,
  getLibraries,
  getPerson,
  getMetadataProviders,
  getLogs,
  getPlaybackDecision,
  getSystemActivities,
  getSettings,
  getStoredAuthToken,
  getUsers,
  searchItems,
  type MediaLibrary,
} from './api';

const app = document.querySelector<HTMLDivElement>('#app');
if (!app) {
  throw new Error('Failed to find app container');
}
const appRoot = app;
let pendingLibraryRefreshHandle: number | undefined;
let pendingMetadataRefreshHandle: number | undefined;
let appStarted = false;

function expandLazyShelfRowsForPatch(root: ParentNode): void {
  root.querySelectorAll<HTMLElement>('[data-lazy-shelf-id]').forEach((row) => {
    const shelfId = row.dataset.lazyShelfId;
    const shelf = shelfId ? state.home?.shelves.find((entry) => entry.id === shelfId) : undefined;
    const currentRow = shelfId
      ? document.querySelector<HTMLElement>(`[data-lazy-shelf-id="${CSS.escape(shelfId)}"]`)
      : undefined;
    if (!shelf || !currentRow) {
      return;
    }

    const currentRenderedCount = Number(currentRow.dataset.lazyRenderedCount ?? currentRow.children.length);
    const nextRenderedCount = Number(row.dataset.lazyRenderedCount ?? row.children.length);
    const targetCount = Math.min(
      shelf.items.length,
      Math.max(
        Number.isFinite(currentRenderedCount) ? currentRenderedCount : currentRow.children.length,
        Number.isFinite(nextRenderedCount) ? nextRenderedCount : row.children.length,
      ),
    );
    if (targetCount > row.children.length) {
      row.insertAdjacentHTML('beforeend', shelf.items.slice(row.children.length, targetCount).map(renderItemCard).join(''));
    }
    row.dataset.lazyRenderedCount = String(targetCount);
    row.dataset.lazyComplete = targetCount >= shelf.items.length ? 'true' : 'false';
  });
}

function setElementHtml(root: HTMLElement, html: string, preserveDom = true): void {
  patchElementHtml(root, html, {
    preserveDom,
    abortEvents: abortRenderEvents,
    beforePatch: expandLazyShelfRowsForPatch,
  });
}

function shouldDeferAutoRefreshRender(): boolean {
  if (state.route.page !== 'item') {
    return false;
  }

  if (state.isPlayerOpen || Boolean(state.activeTrailer)) {
    return true;
  }

  const themeAudio = document.querySelector<HTMLAudioElement>('#theme-song-player');
  if (themeAudio && !themeAudio.paused && !themeAudio.ended) {
    return true;
  }

  return Boolean(document.querySelector('#theme-song-youtube-player'));
}

function maybeRenderAfterAutoRefresh(shouldRender: boolean): void {
  if (state.error) {
    state.hasDeferredAutoRefreshRender = false;
    render();
    return;
  }

  if (state.hasDeferredAutoRefreshRender && !shouldAutoRefreshMetadata()) {
    state.hasDeferredAutoRefreshRender = false;
    render();
    return;
  }

  if (!shouldRender) {
    return;
  }

  if (shouldDeferAutoRefreshRender()) {
    state.hasDeferredAutoRefreshRender = true;
    return;
  }

  state.hasDeferredAutoRefreshRender = false;
  render();
}

function clearPendingLibraryRefresh(): void {
  if (pendingLibraryRefreshHandle !== undefined) {
    globalThis.clearTimeout(pendingLibraryRefreshHandle);
    pendingLibraryRefreshHandle = undefined;
  }
}

function shouldAutoRefreshLibraries(): boolean {
  return state.route.page === 'home'
    && state.libraries.some((library) => library.status === 'never_scanned');
}

function schedulePendingLibraryRefresh(): void {
  clearPendingLibraryRefresh();
  if (!shouldAutoRefreshLibraries()) {
    return;
  }

  pendingLibraryRefreshHandle = globalThis.setTimeout(() => {
    pendingLibraryRefreshHandle = undefined;
    void refreshPendingLibraryData();
  }, 1800);
}

function clearPendingMetadataRefresh(): void {
  if (pendingMetadataRefreshHandle !== undefined) {
    globalThis.clearTimeout(pendingMetadataRefreshHandle);
    pendingMetadataRefreshHandle = undefined;
  }
}

function itemPageMetadataRefreshItemIds(): Set<number> {
  const itemIds = new Set<number>();
  if (state.route.page !== 'item' || !state.selectedItem) {
    return itemIds;
  }

  itemIds.add(state.selectedItem.id);
  state.selectedItem.children.forEach((child) => itemIds.add(child.id));
  state.selectedItem.hierarchy.forEach((ancestor) => itemIds.add(ancestor.id));
  return itemIds;
}

function librariesHavePendingMetadataRefresh(): boolean {
  return state.libraries.some((library) => library.metadata_refresh_pending > 0);
}

function shouldAutoRefreshMetadata(): boolean {
  if (activeLibraryScanActivities().length > 0) {
    return true;
  }

  if (state.route.page === 'settings') {
    return false;
  }

  if (state.route.page === 'item') {
    const itemPageIds = itemPageMetadataRefreshItemIds();
    const hasActiveItemPageRefresh = activeMetadataRefreshActivities()
      .some((activity) => activity.item_ids.some((itemId) => itemPageIds.has(itemId)));

    return itemIsMetadataPending(state.selectedItem)
      || hasActiveItemPageRefresh
      || Boolean(state.selectedItem?.children.some((child) => itemIsMetadataPending(child)))
      || Boolean(state.selectedItemMetadata?.matches.some((match) => match.refresh_state === 'pending'));
  }

  if (activeMetadataRefreshActivities().length > 0) {
    return true;
  }

  if (librariesHavePendingMetadataRefresh()) {
    return true;
  }

  const visibleShelfItems = state.home?.shelves.flatMap((shelf) => shelf.items) ?? [];
  return [...state.libraryItems, ...searchResultItems(), ...visibleShelfItems]
    .some((item) => item.metadata_refresh_state === 'pending');
}

function schedulePendingMetadataRefresh(force = false): void {
  clearPendingMetadataRefresh();
  if (!force && !shouldAutoRefreshMetadata()) {
    return;
  }

  pendingMetadataRefreshHandle = globalThis.setTimeout(() => {
    pendingMetadataRefreshHandle = undefined;
    void refreshPendingMetadataData();
  }, 1500);
}

function navigateTo(path: string, replace = false): void {
  const currentPath = `${globalThis.location.pathname}${globalThis.location.search}`;
  if (currentPath === path) {
    state.route = parseRoute();
    render();
    return;
  }

  if (replace) {
    globalThis.history.replaceState({}, '', path);
  } else {
    globalThis.history.pushState({}, '', path);
  }
  state.route = parseRoute();
  if (state.route.page === 'home') {
    state.homeTab = defaultHomeTab(state.route);
    state.browseFilter = undefined;
  }
  state.isTrailerMenuOpen = false;
  void refreshData();
}

function eventBindingContext(): AppEventBindingContext {
  return {
    navigateTo,
    refreshData,
    refreshPendingMetadataData,
    render,
    schedulePendingMetadataRefresh,
    setElementHtml,
  };
}

function renderLibraryRefreshIndicator(library: MediaLibrary): string {
  const progress = libraryRefreshProgress(library);
  if (!progress) {
    return '';
  }

  const stalePending = Math.max(0, library.metadata_refresh_pending - activeLibraryPendingRefreshCount(library.id));
  const tooltipParts = [`Metadata refresh progress: ${progress.completed}/${progress.total}`];
  if (progress.failed > 0) {
    tooltipParts.push(`${progress.failed} failed`);
  }
  if (stalePending > 0) {
    tooltipParts.push(`${stalePending} pending without active worker`);
  }
  const tooltip = tooltipParts.join(' · ');
  return `
    <span class="library-refresh-indicator" title="${escapeHtml(tooltip)}" aria-label="${escapeHtml(tooltip)}">
      <span class="library-refresh-ring" style="--library-refresh-progress: ${progress.percent}%;"></span>
    </span>
  `;
}

function isRailCollapsed(): boolean {
  return state.route.page === 'item';
}

async function loadLibraryItemsForCurrentRoute(): Promise<void> {
  const route = parseRoute();
  if (route.page !== 'home' && route.page !== 'browse-detail') {
    return;
  }
  const libraryId = route.libraryId;
  const searchQuery = state.searchQuery.trim();
  state.libraryItemsLoading = true;
  render(true);

  try {
    const [libraryItems, searchResults] = await Promise.all([
      getItems(libraryId),
      searchQuery ? searchItems(searchQuery) : Promise.resolve([]),
    ]);
    const nextRoute = parseRoute();
    if (
      (nextRoute.page !== 'home' && nextRoute.page !== 'browse-detail')
      || nextRoute.libraryId !== libraryId
    ) {
      return;
    }
    state.libraryItems = libraryItems;
    state.searchResults = searchResults;
    state.error = undefined;
  } catch (error) {
    state.error = error instanceof Error ? error.message : 'Failed to load library items.';
  } finally {
    const nextRoute = parseRoute();
    if (
      (nextRoute.page === 'home' || nextRoute.page === 'browse-detail')
      && nextRoute.libraryId === libraryId
    ) {
      state.libraryItemsLoading = false;
      render(true);
    }
  }
}

function renderCurrentPage(): string {
  switch (state.route.page) {
    case 'item':
      return renderItemPage();
    case 'person':
      return renderPersonPage();
    case 'settings':
      return renderSettingsPage();
    default:
      return renderHomePage();
  }
}

function renderRail(): string {
  const activeLibraryIdValue = activeLibraryId();
  const collapsed = isRailCollapsed();

  return `
    <aside class="library-rail${collapsed ? ' collapsed' : ''}">
      <div class="library-rail-top">
        <div class="brand-block">
          <div class="brand-mark logo-brand-mark"><img class="brand-logo" src="${escapeHtml(kokoLogoUrl)}" alt="" /></div>
          <div>
            <h1>Koko</h1>
            ${state.apiMode === 'mock' ? '<p>Mock data</p>' : ''}
          </div>
        </div>
        <nav class="rail-nav">
          <button class="rail-button ${state.route.page === 'home' && activeLibraryIdValue === undefined ? 'active' : ''}" type="button" data-nav-home title="Home">
            ${renderIcon('house')}
            <span class="rail-label">Home</span>
          </button>
          ${state.libraries
            .map((library) => `
              <button class="rail-button ${activeLibraryIdValue === library.id ? 'active' : ''}" type="button" data-nav-library-id="${library.id}" title="${escapeHtml(library.name)}">
                ${renderIcon(selectedLibraryIcon(library.kind))}
                <span class="rail-library-copy">
                  <span class="rail-label">${escapeHtml(library.name)}</span>
                  ${renderLibraryRefreshIndicator(library)}
                </span>
              </button>
            `)
            .join('')}
        </nav>
      </div>
      <div class="library-rail-bottom">
        ${currentUser() ? `
          <div class="rail-user-card">
            ${renderUserAvatar(currentUser()!, 'rail-avatar')}
            <span class="rail-user-copy">
              <strong>${escapeHtml(currentUser()!.username)}</strong>
              <span>${currentUser()!.admin ? 'Administrator' : 'Signed in'}</span>
            </span>
          </div>
        ` : ''}
        <button class="rail-button rail-settings ${state.route.page === 'settings' ? 'active' : ''}" type="button" data-nav-settings title="Settings">
          ${renderIcon('settings')}
          <span class="rail-label">Settings</span>
        </button>
        <button class="rail-button" type="button" data-sign-out title="Sign out">
          ${renderIcon('log-out')}
          <span class="rail-label">Sign out</span>
        </button>
      </div>
    </aside>
  `;
}

function render(preserveScroll = true): void {
  if (!state.isPlayerOpen && !state.activeTrailer) {
    document.body.style.cursor = '';
  }

  if (!state.bootstrap && state.isLoading) {
    setElementHtml(appRoot, renderAuthShell('Loading Koko', 'Checking server state and account access.', ''), preserveScroll);
    createIcons({ icons });
    return;
  }

  if (requiresSetup()) {
    setElementHtml(appRoot, renderWelcomeScreen(), preserveScroll);
    createIcons({ icons });
    bindEvents(eventBindingContext());
    return;
  }

  if (requiresLogin()) {
    setElementHtml(appRoot, renderLoginScreen(), preserveScroll);
    createIcons({ icons });
    bindEvents(eventBindingContext());
    return;
  }

  const homeFeature = state.route.page === 'home' || state.route.page === 'browse-detail'
    ? homeFeaturePreview()
    : undefined;
  const pageBackdropUrl = state.route.page === 'item' && state.selectedItem
    && (state.selectedItem.backdrop_url || state.selectedItemMetadata?.matches.some((match) => Boolean(match.backdrop_url || match.cached_backdrop_path)))
      ? getArtworkUrl(state.selectedItem.id, 'backdrop', state.selectedItem.artwork_updated_at)
      : pageBackdropUrlForHomePreview(homeFeature);
  const railCollapsed = isRailCollapsed();
  const pageBackdropScopeClass = state.route.page === 'home' || state.route.page === 'browse-detail'
    ? ' home-page-backdrop'
    : '';

  setElementHtml(appRoot, `
    <div class="app-shell${pageBackdropUrl ? ' has-page-backdrop' : ''}${pageBackdropScopeClass}${railCollapsed ? ' rail-collapsed' : ''}">
      ${pageBackdropUrl ? `<div class="page-backdrop" style="--page-backdrop-image: url('${escapeHtml(pageBackdropUrl)}');"></div>` : ''}
      ${renderRail()}
      <div class="main-shell">
        <div class="main-shell-inner">
          ${state.error ? `<section class="panel error-panel page-panel">${escapeHtml(state.error)}</section>` : ''}
          ${renderCurrentPage()}
        </div>
      </div>
      ${renderPlayerOverlay()}
    </div>
  `, preserveScroll);

  createIcons({ icons });
  bindEvents(eventBindingContext());
  syncVisibleSpinners();
  syncThemeSongPlayer();
}

async function refreshData(showLoading = true): Promise<void> {
  state.route = parseRoute();
  state.isLoading = true;
  state.error = undefined;
  state.apiMode = getApiMode();
  if (showLoading) {
    render(false);
  }

  try {
    state.bootstrap = await getAppBootstrap().catch(async (error) => {
      if (!getStoredAuthToken()) {
        return Promise.reject(error);
      }

      clearStoredAuthToken();
      return getAppBootstrap();
    });

    if (requiresSetup() || requiresLogin()) {
      clearPendingLibraryRefresh();
      clearPendingMetadataRefresh();
      state.capabilities = undefined;
      state.libraries = [];
      state.home = undefined;
      state.libraryItems = [];
      state.searchResults = [];
      state.showFullSearchResults = false;
      state.metadataProviders = [];
      state.systemActivities = [];
      state.dashboardItems = [];
      state.settingsResponse = undefined;
      state.logsResponse = undefined;
      state.selectedItem = undefined;
      state.selectedItemMetadata = undefined;
      state.selectedPerson = undefined;
      state.selectedPlayback = undefined;
      state.metadataSearchResults = [];
      state.users = [];
      state.hasDeferredAutoRefreshRender = false;
      return;
    }

    const [capabilities, libraries, metadataProviders, settingsResponse, systemActivitiesResponse] = await Promise.all([
      getCapabilities(),
      getLibraries(),
      getMetadataProviders(),
      getSettings(),
      getSystemActivities(),
    ]);

    state.capabilities = capabilities;
    state.libraries = libraries;
    state.metadataProviders = metadataProviders;
    state.settingsResponse = settingsResponse;
    state.systemActivities = systemActivitiesResponse.activities;
    state.users = canManageUsers() ? await getUsers() : [];

    if (state.route.page === 'home' || state.route.page === 'browse-detail') {
      const libraryId = state.route.libraryId;
      const home = await getHome(libraryId);
      state.home = home;
      state.libraryItems = [];
      state.searchResults = [];
      state.libraryItemsLoading = true;
      state.selectedItem = undefined;
      state.selectedItemMetadata = undefined;
      state.selectedPerson = undefined;
      state.selectedPlayback = undefined;
      state.metadataSearchResults = [];
      state.metadataSearchQuery = '';
      state.metadataSearchYear = '';
      state.metadataSearchLanguage = '';
      state.metadataSearchProviders = [];
      state.isPlayerOpen = false;
      state.activePlaybackItem = undefined;
      state.activePlaybackSession = undefined;
      state.activePlaybackStartMs = 0;
      state.activeAudioStreamIndex = undefined;
      state.isAudioTrackMenuOpen = false;
      state.isTrailerMenuOpen = false;
      state.activeTrailer = undefined;
      state.hasDeferredAutoRefreshRender = false;
      state.dashboardItems = [];
      state.logsResponse = undefined;
      void loadLibraryItemsForCurrentRoute();
    } else if (state.route.page === 'item') {
      state.home = undefined;
      state.libraryItems = [];
      state.libraryItemsLoading = false;
      state.searchResults = [];
      state.showFullSearchResults = false;
      state.metadataSearchResults = [];
      state.metadataSearchQuery = '';
      state.metadataSearchYear = '';
      state.metadataSearchLanguage = '';
      state.metadataSearchProviders = [];
      state.isTrailerMenuOpen = false;
      state.activeTrailer = undefined;
      state.dashboardItems = [];
      state.logsResponse = undefined;
      const [item, metadata, playback] = await Promise.all([
        getItem(state.route.itemId),
        getItemMetadata(state.route.itemId),
        getPlaybackDecision(state.route.itemId),
      ]);
      const [home, libraryItems] = await Promise.all([
        getHome(item.library_id),
        getItems(item.library_id),
      ]);
      state.home = home;
      state.libraryItems = libraryItems;
      state.selectedItem = item;
      state.selectedItemMetadata = metadata;
      state.selectedPlayback = playback;
      state.selectedPerson = undefined;
    } else if (state.route.page === 'person') {
      state.home = undefined;
      state.libraryItems = [];
      state.libraryItemsLoading = false;
      state.searchResults = [];
      state.showFullSearchResults = false;
      state.selectedItem = undefined;
      state.selectedItemMetadata = undefined;
      state.selectedPlayback = undefined;
      state.metadataSearchResults = [];
      state.metadataSearchQuery = '';
      state.metadataSearchYear = '';
      state.metadataSearchLanguage = '';
      state.metadataSearchProviders = [];
      state.isPlayerOpen = false;
      state.activePlaybackItem = undefined;
      state.activePlaybackSession = undefined;
      state.activePlaybackStartMs = 0;
      state.activeAudioStreamIndex = undefined;
      state.isAudioTrackMenuOpen = false;
      state.isTrailerMenuOpen = false;
      state.activeTrailer = undefined;
      state.dashboardItems = [];
      state.logsResponse = undefined;
      state.selectedPerson = await getPerson(state.route.personId);
    } else {
      state.home = undefined;
      state.libraryItems = [];
      state.libraryItemsLoading = false;
      state.searchResults = [];
      state.showFullSearchResults = false;
      state.selectedItem = undefined;
      state.selectedItemMetadata = undefined;
      state.selectedPerson = undefined;
      state.selectedPlayback = undefined;
      state.metadataSearchResults = [];
      state.metadataSearchQuery = '';
      state.metadataSearchYear = '';
      state.metadataSearchLanguage = '';
      state.metadataSearchProviders = [];
      state.isPlayerOpen = false;
      state.activePlaybackItem = undefined;
      state.activePlaybackSession = undefined;
      state.activePlaybackStartMs = 0;
      state.activeAudioStreamIndex = undefined;
      state.isAudioTrackMenuOpen = false;
      state.isTrailerMenuOpen = false;
      state.activeTrailer = undefined;
      state.hasDeferredAutoRefreshRender = false;
      if (state.route.section === 'dashboard') {
        state.logsResponse = undefined;
        state.dashboardItems = await getItems();
      } else if (state.route.section === 'logs') {
        state.dashboardItems = [];
        state.logsResponse = await getLogs(currentLogFilterRequest());
      } else {
        state.dashboardItems = [];
        state.logsResponse = undefined;
      }
    }

    state.apiMode = getApiMode();
  } catch (error) {
    state.error = error instanceof Error ? error.message : 'Failed to load server data.';
    state.apiMode = getApiMode();
  } finally {
    state.isLoading = false;
    schedulePendingLibraryRefresh();
    schedulePendingMetadataRefresh();
    render(true);
  }
}

async function refreshPendingLibraryData(): Promise<void> {
  const route = parseRoute();
  if (route.page !== 'home') {
    return;
  }

  let shouldRender = false;
  const previousError = state.error;

  try {
    const libraryId = route.libraryId;
    const searchQuery = state.searchQuery.trim();
    const previousSnapshot = snapshotJson({
      libraries: state.libraries,
      home: state.home,
      libraryItems: state.libraryItems,
      searchResults: state.searchResults,
    });
    const [libraries, home, libraryItems, searchResults] = await Promise.all([
      getLibraries(),
      getHome(libraryId),
      getItems(libraryId),
      searchQuery
        ? searchItems(searchQuery)
        : Promise.resolve([]),
    ]);
    if (state.route.page !== 'home' || state.route.libraryId !== libraryId) {
      return;
    }

    state.libraries = libraries;
    state.home = home;
    state.libraryItems = libraryItems;
    state.searchResults = searchResults;
    state.error = undefined;
    shouldRender = previousSnapshot !== snapshotJson({
      libraries: state.libraries,
      home: state.home,
      libraryItems: state.libraryItems,
      searchResults: state.searchResults,
    }) || previousError !== state.error;
  } catch (error) {
    state.error = error instanceof Error ? error.message : 'Failed to refresh library data.';
    shouldRender = previousError !== state.error;
  } finally {
    schedulePendingLibraryRefresh();
    maybeRenderAfterAutoRefresh(shouldRender);
  }
}

async function refreshPendingMetadataData(): Promise<void> {
  const route = parseRoute();
  let shouldRender = false;
  const previousError = state.error;

  try {
    if (route.page === 'item') {
      const itemId = route.itemId;
      const previousSnapshot = snapshotJson({
        systemActivities: state.systemActivities,
        libraries: state.libraries,
        home: state.home,
        libraryItems: state.libraryItems,
        selectedItem: state.selectedItem,
        selectedItemMetadata: state.selectedItemMetadata,
      });
      const [activitiesResponse, libraries, item, metadata] = await Promise.all([
        getSystemActivities(),
        getLibraries(),
        getItem(itemId),
        getItemMetadata(itemId),
      ]);
      if (state.route.page !== 'item' || state.route.itemId !== itemId) {
        return;
      }
      const [home, libraryItems] = await Promise.all([
        getHome(item.library_id),
        getItems(item.library_id),
      ]);
      if (state.route.page !== 'item' || state.route.itemId !== itemId) {
        return;
      }

      state.systemActivities = activitiesResponse.activities;
      state.libraries = libraries;
      state.home = home;
      state.libraryItems = libraryItems;
      state.selectedItem = item;
      state.selectedItemMetadata = metadata;
      state.error = undefined;
      shouldRender = previousSnapshot !== snapshotJson({
        systemActivities: state.systemActivities,
        libraries: state.libraries,
        home: state.home,
        libraryItems: state.libraryItems,
        selectedItem: state.selectedItem,
        selectedItemMetadata: state.selectedItemMetadata,
      }) || previousError !== state.error;
    } else if (route.page === 'home') {
      const libraryId = route.libraryId;
      const searchQuery = state.searchQuery.trim();
      const previousSnapshot = snapshotJson({
        systemActivities: state.systemActivities,
        libraries: state.libraries,
        home: state.home,
        libraryItems: state.libraryItems,
        searchResults: state.searchResults,
      });
      const [activitiesResponse, libraries, home, libraryItems, searchResults] = await Promise.all([
        getSystemActivities(),
        getLibraries(),
        getHome(libraryId),
        getItems(libraryId),
        searchQuery
          ? searchItems(searchQuery)
          : Promise.resolve([]),
      ]);
      if (state.route.page !== 'home' || state.route.libraryId !== libraryId) {
        return;
      }

      state.systemActivities = activitiesResponse.activities;
      state.libraries = libraries;
      state.home = home;
      state.libraryItems = libraryItems;
      state.searchResults = searchResults;
      state.error = undefined;
      shouldRender = previousSnapshot !== snapshotJson({
        systemActivities: state.systemActivities,
        libraries: state.libraries,
        home: state.home,
        libraryItems: state.libraryItems,
        searchResults: state.searchResults,
      }) || previousError !== state.error;
    } else {
      const previousSnapshot = snapshotJson({
        systemActivities: state.systemActivities,
        libraries: state.libraries,
        logsResponse: state.logsResponse,
        dashboardItems: state.dashboardItems,
      });
      const [activitiesResponse, libraries, logsResponse, dashboardItems] = await Promise.all([
        getSystemActivities(),
        getLibraries(),
        getLogs(currentLogFilterRequest()),
        getItems(),
      ]);
      state.systemActivities = activitiesResponse.activities;
      state.libraries = libraries;
      state.logsResponse = logsResponse;
      state.dashboardItems = dashboardItems;
      state.error = undefined;
      shouldRender = previousSnapshot !== snapshotJson({
        systemActivities: state.systemActivities,
        libraries: state.libraries,
        logsResponse: state.logsResponse,
        dashboardItems: state.dashboardItems,
      }) || previousError !== state.error;
    }
  } catch (error) {
    state.error = error instanceof Error ? error.message : 'Failed to refresh media metadata.';
    shouldRender = previousError !== state.error;
  } finally {
    schedulePendingMetadataRefresh();
    maybeRenderAfterAutoRefresh(shouldRender);
  }
}

/** Starts the browser UI, binds global controls, and loads the initial data set. */
export function startApp(): void {
  if (appStarted) {
    return;
  }

  appStarted = true;
  configurePlaybackController({ render, refreshData });
  bindGlobalInputHandlers(state);

  globalThis.addEventListener('popstate', () => {
    state.route = parseRoute();
    if (state.route.page === 'home' || state.route.page === 'browse-detail') {
      state.homeTab = defaultHomeTab(state.route);
      state.browseFilter = undefined;
    }
    state.isTrailerMenuOpen = false;
    void refreshData();
  });

  render();
  void refreshData();
}
