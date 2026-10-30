/** Binds DOM events for each rendered app tree and owns transient UI timers. */

import { clearBrowseSessions } from './browseSession';
import { bindLiveTvActions } from './liveTvActions';
import { dismissPlayer } from './playbackController';
import { bindPodcastActions, bindPodcastAddActions, bindPodcastDirectoryPreviews, retryPodcastAction } from './podcastActions';
import { renderPodcastFeature } from './podcasts';
import { bindBookActions } from './bookActions';
import { queueMusicItem, startMusicRadio, playMusicPlaylist, resumeMusicQueue, stopMusic, toggleCurrentMusic } from './audioPlayer';
import { bindSongMenus } from './audioPlayerSupport';
import { createIcons, icons } from 'lucide';
import { HOME_SHELF_CHUNK_SIZE } from './constants';
import { currentLogFilterRequest, libraryHasActiveMetadataRefresh } from './activities';
import { bindProfileImagePickers, readProfileImageUpload } from './auth';
import { renderMetadataDashboard } from './dashboardView';
import { LOG_PAGE_SIZE, logsAreLoading, renderLogResults } from './logView';
import { DEFAULT_BROWSE_PREFERENCES, normalizeBrowsePreferences, saveBrowsePreferences } from './browsePreferences';
import { bindHoverPreview } from './hoverPreview';
import { bindItemOpeningLayout, bindItemPlaybackBar } from './itemLayout';
import { bindResumeHold } from './resumeHold';
import { bindAudiobookActions } from './audiobookActions';
import { bindSettingsActions } from './settingsActions';
import { bindProviderOrdering, syncProviderDependencyOptions } from './providerOrdering';
import { escapeHtml } from './format';
import { formDataString, formDataStrings, normalizedMetadataLanguages, parseMetadataLanguageInput, parsePathsInput } from './formUtils';
import { mediaExtraToTrailerOption } from './mediaExtras';
import { currentThemeSongYouTubeTarget, currentTrailerOptions } from './mediaTargets';
import {
  bindPlayerProgress,
  bindTrailerPlayer,
  closeActivePlaybackSession,
  closeTrailerPlayer,
  openTrailer,
  openVideoOverlay,
  playYouTubeThemeSong,
  startPlayback,
  startMusicPlayback,
  startPlaybackForItemId,
} from './playbackController';
import { parseRoute } from './routes';
import {
  activeLibrary,
  activeLibraryId,
  backNavigationTarget,
  categorySummaries,
  collectionSummaries,
  homePreviewCandidates,
  pageBackdropUrlForCollection,
  pageBackdropUrlForItem,
  searchResultCollections,
  showPreviewItemForHighlight,
} from './selectors';
import { syncVisibleSpinners } from './spinners';
import { state } from './state';
import { mutateMetadata } from './metadataRequests';
import type { HomeBrowseTab } from './types';
import { browseDetailPath, homeBrowsePath, homeShelfEntries, renderHomeShelfEntry, renderHomeFeature, LIBRARY_PAGE_SIZE, visibleLibraryItems } from './homeView';
import {
  bindPersonCreditTrays,
  defaultMetadataSearchLanguage,
  selectedItemDefaultMetadataTitle,
  selectedItemDefaultMetadataYear,
  selectedItemExtras,
} from './itemPersonView';
import { buildSettingsFromForm, buildPodcastUserSettingsFromForm } from './settingsView';
import { setButtonBusy } from './ui';
import {
  addLibrary,
  clearMetadataCache,
  clearStoredAuthToken,
  logoutUser,
  createUser,
  deleteLibrary,
  deleteMissingItems,
  getLogs,
  getUsers,
  getAudiobookRecordings,
  deletePlaylist,
  linkItemMetadata,
  unlinkItemMetadata,
  loginUser,
  refreshItemMetadata,
  refreshLibraryMetadata,
  runScheduledTask,
  scanLibrary,
  searchItemMetadata,
  searchItems,
  setStoredAuthToken,
  updateSettings,
  setPodcastUserSettings,
  updateUser,
  type CreateUserRequest,
  type LoginRequest,
  type MediaLibrarySettings,
  type MediaShelf,
  type ScheduledTaskId,
  type UpdateUserRequest,
} from '../api';

/** Replaces a DOM subtree while preserving any coordinator-level patch behavior. */
export type ReplaceElementHtml = (root: HTMLElement, html: string, preserveDom?: boolean) => void;

/** Coordinator callbacks required by render-scoped event handlers. */
export interface AppEventBindingContext {
  /** Navigates to an app route and optionally replaces the current history entry. */
  navigateTo: (path: string, replace?: boolean) => void;
  /** Reloads bootstrap and route data. */
  refreshData: (showLoading?: boolean) => Promise<void>;
  /** Refreshes metadata-dependent route data after an item or library action. */
  refreshPendingMetadataData: () => Promise<void>;
  /** Re-renders the app shell. */
  render: (preserveScroll?: boolean) => void;
  /** Schedules the background metadata refresh poller. */
  schedulePendingMetadataRefresh: (force?: boolean) => void;
  /** Replaces a partial DOM subtree and reuses the app DOM patcher. */
  setElementHtml: ReplaceElementHtml;
}

let activeBindingContext: AppEventBindingContext | undefined;

function navigateTo(path: string, replace?: boolean): void {
  if (!activeBindingContext) {
    throw new Error('Event binding context has not been initialized.');
  }
  activeBindingContext.navigateTo(path, replace);
}

/** Aborts listeners bound to the previous rendered DOM tree. */
export function abortRenderEvents(): void {
  renderEventController?.abort();
}

/** Debounce handle for incremental home search updates. */
let pendingLiveSearchHandle: number | undefined;
let liveSearchGeneration = 0;

/** Navigation invalidates both queued searches and replies from the previous page. */
export function cancelPendingHomeSearch(): void {
  liveSearchGeneration += 1;
  if (pendingLiveSearchHandle !== undefined) {
    globalThis.clearTimeout(pendingLiveSearchHandle);
    pendingLiveSearchHandle = undefined;
  }
  state.searchLoading = false;
}

function clearHomeSearch(): boolean {
  const hadSearch = Boolean(state.searchQuery) || state.searchResults.length > 0 || state.showFullSearchResults;
  cancelPendingHomeSearch();
  state.searchQuery = '';
  state.searchResults = [];
  state.showFullSearchResults = false;
  return hadSearch;
}

/** Live search refreshes its results without reloading settings or the media library. */
async function refreshHomeSearch(context: AppEventBindingContext): Promise<void> {
  if (pendingLiveSearchHandle !== undefined) clearTimeout(pendingLiveSearchHandle);
  pendingLiveSearchHandle = undefined;
  const generation = ++liveSearchGeneration;
  const query = state.searchQuery.trim();
  const route = state.route;
  state.searchLoading = Boolean(query);
  if (!query) {
    state.searchResults = [];
    context.render(true);
    return;
  }
  context.render(true);
  try {
    const results = await searchItems(query);
    if (generation === liveSearchGeneration && state.route === route && state.searchQuery.trim() === query) state.searchResults = results;
  } catch (error) {
    if (generation === liveSearchGeneration && state.route === route && state.searchQuery.trim() === query) {
      state.error = error instanceof Error ? error.message : 'Failed to search the library.';
    }
  } finally {
    if (generation === liveSearchGeneration && state.route === route && state.searchQuery.trim() === query) {
      state.searchLoading = false;
      context.render(true);
    }
  }
}

/** Tracks the one global resize listener used to refresh shelf controls. */
let shelfScrollResizeBound = false;

/** Controller for DOM event listeners attached during the latest render. */
let renderEventController: AbortController | undefined;



function setAuthFormBusy(form: HTMLFormElement, busy: boolean): void {
  form.querySelectorAll<HTMLInputElement | HTMLButtonElement>('input, button').forEach((control) => {
    control.disabled = busy;
  });
}

/** Share account field parsing between initial setup and administrator-created users. */
async function createUserRequest(form: HTMLFormElement, firstUser = false): Promise<CreateUserRequest> {
  const formData = new FormData(form);
  return {
    username: formDataString(formData.get('username')).trim(),
    password: formDataString(formData.get('password')),
    pin: formDataString(formData.get('pin')).trim() || undefined,
    admin: firstUser || formData.get('admin') === 'on',
    birthday: formDataString(formData.get('birthday')).trim() || undefined,
    profile_image_upload: await readProfileImageUpload(formData),
    preferred_metadata_languages: parseMetadataLanguageInput(formData.get('preferred_metadata_languages')),
  };
}



/** Refresh results without rebuilding the filter form or rebinding the entire app. */
async function refreshLogsView(context: AppEventBindingContext): Promise<void> {
  if (state.route.page !== 'settings' || state.route.section !== 'logs' || logsAreLoading()) {
    return;
  }
  const route = state.route;
  state.logsLoading = true;
  state.logsError = undefined;
  updateLogResults(context);
  try {
    const response = await getLogs(currentLogFilterRequest());
    if (state.route === route) {
      state.logsResponse = response;
      state.logPage = 0;
    }
  } catch (error) {
    if (state.route === route) {
      state.logsError = error instanceof Error ? error.message : 'Failed to load logs.';
    }
  } finally {
    state.logsLoading = false;
    if (state.route.page === 'settings' && state.route.section === 'logs') {
      updateLogResults(context);
    }
  }
}

/** Patch the bounded result list and synchronize controls while a request is running. */
function updateLogResults(context: AppEventBindingContext): void {
  const root = document.querySelector<HTMLElement>('#log-results');
  if (!root) {
    return;
  }
  context.setElementHtml(root, renderLogResults());
  createIcons({ icons, root });
  document.querySelectorAll<HTMLButtonElement>('#refresh-log-viewer, #log-filter-form button').forEach((button) => {
    button.disabled = logsAreLoading();
  });
  const path = document.querySelector<HTMLElement>('.log-file-path code');
  if (path) {
    path.textContent = state.logsResponse?.log_path ?? 'Unavailable';
  }
}


function updatePageBackdrop(backdropUrl: string | undefined): void {
  const appShell = document.querySelector<HTMLElement>('.app-shell');
  const pageBackdrop = document.querySelector<HTMLElement>('.page-backdrop');
  if (backdropUrl) {
    appShell?.classList.add('has-page-backdrop');
    if (pageBackdrop) {
      const escapedBackdropUrl = backdropUrl.replace(/'/g, String.raw`\'`);
      pageBackdrop.style.setProperty('--page-backdrop-image', `url('${escapedBackdropUrl}')`);
    } else {
      appShell?.insertAdjacentHTML('afterbegin', `<div class="page-backdrop" style="--page-backdrop-image: url('${escapeHtml(backdropUrl)}');"></div>`);
    }
  } else {
    appShell?.classList.remove('has-page-backdrop');
    pageBackdrop?.remove();
  }
}



function bindHomeFeatureAction(): void {
  document.querySelector<HTMLElement>('.home-feature [data-item-id]')?.addEventListener('click', () => {
    const nextItemId = Number(document.querySelector<HTMLElement>('.home-feature [data-item-id]')?.dataset.itemId);
    if (Number.isFinite(nextItemId)) {
      navigateTo(`/items/${nextItemId}`);
    }
  });

  document.querySelector<HTMLElement>('.home-feature [data-collection-filter]')?.addEventListener('click', () => {
    const collectionId = document.querySelector<HTMLElement>('.home-feature [data-collection-filter]')?.dataset.collectionFilter;
    if (collectionId) {
      navigateTo(browseDetailPath('collection', collectionId));
    }
  });
}



function refreshHomeFeatureElement(backdropUrl: string | undefined): void {
  const root = document.querySelector<HTMLElement>('.home-feature');
  if (root) {
    root.outerHTML = state.route.page === 'podcasts' ? renderPodcastFeature() : renderHomeFeature();
    createIcons({ icons });
    bindHomeFeatureAction();
    const feature = document.querySelector<HTMLElement>('.home-feature');
    if (state.route.page === 'podcasts' && feature && activeBindingContext) {
      bindPodcastAddActions(feature, activeBindingContext, renderEventController?.signal);
    }
  }
  updatePageBackdrop(backdropUrl);
}



function activatePreviewItem(itemId: number): void {
  if (state.route.page === 'browse-detail' || !Number.isFinite(itemId) || state.homePreviewItemId === itemId) {
    return;
  }
  state.homePreviewItemId = itemId;
  state.homePreviewCollectionId = undefined;
  if (state.route.page === 'podcasts') {
    refreshHomeFeatureElement(undefined);
    return;
  }
  const highlightedItem = homePreviewCandidates().find((item) => item.id === itemId);
  const previewItem = highlightedItem ? showPreviewItemForHighlight(highlightedItem) : undefined;
  refreshHomeFeatureElement(pageBackdropUrlForItem(previewItem));
}



function activatePreviewCollection(collectionId: string | undefined): void {
  if (!collectionId || state.homePreviewCollectionId === collectionId) {
    return;
  }
  state.homePreviewCollectionId = collectionId;
  state.homePreviewItemId = undefined;
  const collection = collectionSummaries().find((entry) => entry.id === collectionId)
    ?? searchResultCollections().find((entry) => entry.id === collectionId);
  refreshHomeFeatureElement(pageBackdropUrlForCollection(collection));
}



function bindItemNavigationElement(element: HTMLElement): void {
  if (element.dataset.boundItemNavigation === 'true') {
    return;
  }
  element.dataset.boundItemNavigation = 'true';
  element.addEventListener('click', () => {
    const itemId = Number(element.dataset.itemId);
    if (!Number.isFinite(itemId)) {
      return;
    }

    navigateTo(`/items/${itemId}`);
  });
}



function bindPreviewItemElement(element: HTMLElement): void {
  if (element.dataset.boundPreviewItem === 'true') {
    return;
  }
  element.dataset.boundPreviewItem = 'true';
  const updatePreview = (): void => {
    activatePreviewItem(Number(element.dataset.previewItemId));
  };
  bindHoverPreview(element, updatePreview);
}



function bindPreviewCollectionElement(element: HTMLElement): void {
  if (element.dataset.boundPreviewCollection === 'true') {
    return;
  }
  element.dataset.boundPreviewCollection = 'true';
  const updatePreview = (): void => {
    activatePreviewCollection(element.dataset.previewCollectionId);
  };
  bindHoverPreview(element, updatePreview);
}



function bindMediaCardInteractions(root: ParentNode): void {
  root.querySelectorAll<HTMLElement>('[data-item-id]').forEach(bindItemNavigationElement);
  root.querySelectorAll<HTMLElement>('[data-preview-item-id]').forEach(bindPreviewItemElement);
  root.querySelectorAll<HTMLElement>('[data-preview-collection-id]').forEach(bindPreviewCollectionElement);
}



function homeShelfForRow(row: HTMLElement): MediaShelf | undefined {
  const shelfId = row.dataset.lazyShelfId;
  const home = state.route.page === 'podcasts' ? state.podcasts?.home : state.home;
  return shelfId ? home?.shelves.find((shelf) => shelf.id === shelfId) : undefined;
}



function appendLazyShelfItems(row: HTMLElement): boolean {
  const shelf = homeShelfForRow(row);
  if (!shelf) {
    return false;
  }

  const renderedCount = Number(row.dataset.lazyRenderedCount ?? row.children.length);
  const entries = homeShelfEntries(shelf, state.route.page === 'podcasts' ? state.podcasts?.home : state.home);
  if (!Number.isFinite(renderedCount) || renderedCount >= entries.length) {
    row.dataset.lazyComplete = 'true';
    return false;
  }

  const nextCount = Math.min(entries.length, renderedCount + HOME_SHELF_CHUNK_SIZE);
  row.insertAdjacentHTML('beforeend', entries.slice(renderedCount, nextCount).map(renderHomeShelfEntry).join(''));
  row.dataset.lazyRenderedCount = String(nextCount);
  row.dataset.lazyComplete = nextCount >= entries.length ? 'true' : 'false';
  bindMediaCardInteractions(row);
  createIcons({ icons });
  syncVisibleSpinners();
  updateShelfScrollControls(row);
  return true;
}



function appendLazyShelfItemsIfNeeded(row: HTMLElement): void {
  const threshold = Math.max(360, row.clientWidth * 0.45);
  while (row.dataset.lazyComplete !== 'true') {
    const remainingScroll = row.scrollWidth - row.scrollLeft - row.clientWidth;
    if (remainingScroll > threshold) {
      return;
    }
    if (!appendLazyShelfItems(row)) {
      return;
    }
  }
}



function parseShelfScrollTarget(value: string | undefined): { shelfId: string; direction: number } | undefined {
  const separatorIndex = value?.lastIndexOf(':') ?? -1;
  if (!value || separatorIndex <= 0) {
    return undefined;
  }

  const shelfId = value.slice(0, separatorIndex);
  const direction = Number(value.slice(separatorIndex + 1));
  return Number.isFinite(direction) ? { shelfId, direction } : undefined;
}



function setShelfScrollButtonVisible(button: HTMLButtonElement | undefined, visible: boolean): void {
  if (!button) {
    return;
  }

  button.classList.toggle('is-scroll-hidden', !visible);
  button.disabled = !visible;
  button.tabIndex = visible ? 0 : -1;
  button.setAttribute('aria-hidden', visible ? 'false' : 'true');
}



function updateShelfScrollControls(row: HTMLElement): void {
  const shelfId = row.dataset.shelfRow;
  const shell = row.closest<HTMLElement>('.shelf-row-shell');
  if (!shelfId || !shell) {
    return;
  }

  const buttons = Array.from(shell.querySelectorAll<HTMLButtonElement>('[data-shelf-scroll]'));
  const leftButton = buttons.find((button) => parseShelfScrollTarget(button.dataset.shelfScroll)?.direction === -1);
  const rightButton = buttons.find((button) => parseShelfScrollTarget(button.dataset.shelfScroll)?.direction === 1);
  const hasOverflow = row.scrollWidth > row.clientWidth + 1;
  const atLeftEdge = row.scrollLeft <= 1;
  const atRightEdge = row.scrollLeft + row.clientWidth >= row.scrollWidth - 1;

  shell.classList.toggle('no-scroll', !hasOverflow);
  setShelfScrollButtonVisible(leftButton, hasOverflow && !atLeftEdge);
  setShelfScrollButtonVisible(rightButton, hasOverflow && !atRightEdge);
}



function refreshShelfScrollControls(): void {
  document.querySelectorAll<HTMLElement>('[data-shelf-row]').forEach(updateShelfScrollControls);
}



function eventListenerOptionsWithSignal(
  options: boolean | AddEventListenerOptions | undefined,
  signal: AbortSignal,
): boolean | AddEventListenerOptions {
  if (typeof options === 'boolean') {
    return { capture: options, signal };
  }

  if (options === undefined) {
    return { signal };
  }

  return { ...options, signal };
}

/** Binds all event handlers for the current rendered DOM tree. */
export function bindEvents(context: AppEventBindingContext): void {
  activeBindingContext = context;
  renderEventController?.abort();
  renderEventController = new AbortController();
  const signal = renderEventController.signal;
  const originalAddEventListener = EventTarget.prototype.addEventListener;
  EventTarget.prototype.addEventListener = function (
    this: EventTarget,
    type: string,
    listener: EventListenerOrEventListenerObject | null,
    options?: boolean | AddEventListenerOptions,
  ): void {
    if (this === globalThis) {
      originalAddEventListener.call(this, type, listener, options);
      return;
    }
    const optionsWithSignal = eventListenerOptionsWithSignal(options, signal);
    originalAddEventListener.call(this, type, listener, optionsWithSignal);
  };

  try {
    bindRenderEvents(context, signal);
  } finally {
    EventTarget.prototype.addEventListener = originalAddEventListener;
  }
}



/** Bind item layout and disclosures, returning a shared way to dismiss secondary actions. */
function bindItemDetailLayout(signal: AbortSignal): () => void {
  const itemPage = document.querySelector<HTMLElement>('.item-detail-page');
  if (itemPage) bindItemOpeningLayout(itemPage, signal, () => {
    itemPage.querySelectorAll<HTMLElement>('[data-item-row]').forEach(updateShelfScrollControls);
  });
  const itemBar = document.querySelector<HTMLElement>('.item-action-dock');
  const itemAnchor = document.querySelector<HTMLElement>('.item-actions-anchor');
  const itemScroller = itemBar?.closest<HTMLElement>('.main-shell');
  if (itemBar && itemAnchor && itemScroller) bindItemPlaybackBar(itemScroller, itemAnchor, itemBar, signal);
  const moreActions = document.querySelector<HTMLDetailsElement>('#item-more');
  const updateMoreActions = (): void => {
    if (!moreActions) return;
    state.itemMoreOpen = moreActions.open;
    const menu = moreActions.querySelector<HTMLElement>('.item-more-menu');
    if (!moreActions.open || !menu || !itemScroller || !itemBar) return;
    menu.style.maxHeight = `${Math.max(48, itemScroller.clientHeight - itemBar.offsetHeight - 16)}px`;
    const overflow = menu.getBoundingClientRect().bottom - itemScroller.getBoundingClientRect().bottom;
    if (overflow > 0) {
      const roomToScroll = Math.max(0, itemBar.getBoundingClientRect().top - itemScroller.getBoundingClientRect().top);
      itemScroller.scrollTop += Math.min(overflow + 8, roomToScroll);
    }
  };
  moreActions?.addEventListener('toggle', updateMoreActions);
  updateMoreActions();
  const closeMoreActions = (): void => {
    if (moreActions) moreActions.open = false;
    state.itemMoreOpen = false;
  };
  if (moreActions) {
    globalThis.addEventListener('resize', updateMoreActions, { signal });
    moreActions.addEventListener('click', (event) => {
      if (event.target instanceof Element && event.target.closest('button')) closeMoreActions();
    }, { capture: true });
    document.addEventListener('click', (event) => {
      if (event.target instanceof Node && !moreActions.contains(event.target)) closeMoreActions();
    });
    document.addEventListener('focusin', (event) => {
      if (event.target instanceof Node && !moreActions.contains(event.target)) closeMoreActions();
    });
  }
  document.querySelector<HTMLButtonElement>('#show-item-details')?.addEventListener('click', () => {
    closeMoreActions();
    const details = document.querySelector<HTMLDetailsElement>('#item-management');
    if (details) {
      details.open = true;
      state.itemManagementOpen = true;
      details.scrollIntoView({ block: 'start' });
      details.querySelector<HTMLElement>('summary')?.focus({ preventScroll: true });
    }
  });
  return closeMoreActions;
}

/** Open a distinct playback-options section without starting playback or rebuilding the page. */
function bindResumeOptions(signal: AbortSignal): () => void {
  const trigger = document.querySelector<HTMLButtonElement>('[data-resume-hold][aria-controls="resume-playback-menu"]');
  const menu = document.querySelector<HTMLElement>('#resume-playback-menu');
  const close = (): void => {
    state.resumeOptionsOpen = false;
    if (menu) menu.hidden = true;
    trigger?.setAttribute('aria-expanded', 'false');
  };
  if (trigger && menu) {
    trigger.addEventListener('resume-options-close', close);
    menu.addEventListener('click', (event) => {
      if (event.target instanceof Element && event.target.closest('button')) close();
    }, { capture: true });
    document.addEventListener('click', (event) => {
      if (event.target instanceof Node && !menu.contains(event.target) && !trigger.contains(event.target)) close();
    });
    document.addEventListener('focusin', (event) => {
      if (event.target instanceof Node && !menu.contains(event.target) && !trigger.contains(event.target)) close();
    });
  }
  return () => {
    if (!trigger || !menu || signal.aborted) return;
    state.resumeOptionsOpen = true;
    menu.hidden = false;
    trigger.setAttribute('aria-expanded', 'true');
    const scroller = menu.closest<HTMLElement>('.main-shell');
    const bar = menu.closest<HTMLElement>('.item-action-dock');
    if (scroller && bar) {
      menu.style.maxHeight = `${Math.max(48, scroller.clientHeight - bar.offsetHeight - 16)}px`;
      const overflow = menu.getBoundingClientRect().bottom - scroller.getBoundingClientRect().bottom;
      if (overflow > 0) scroller.scrollTop += Math.min(overflow + 8, Math.max(0, bar.getBoundingClientRect().top - scroller.getBoundingClientRect().top));
    }
    menu.querySelector<HTMLButtonElement>('button')?.focus({ preventScroll: true });
  };
}

function bindRenderEvents(context: AppEventBindingContext, signal: AbortSignal): void {
  bindBookActions(signal, (error: unknown) => {
      state.error = error instanceof Error ? error.message : 'The reader could not start.'; context.render();
  });
  bindAudiobookActions(signal, (error: unknown) => {
    state.error = error instanceof Error ? error.message : 'The audiobook could not start.'; context.render();
  });
  const { navigateTo, refreshData, refreshPendingMetadataData, render, schedulePendingMetadataRefresh, setElementHtml } = context;
  bindPodcastActions(signal, context);
  bindLiveTvActions(signal, context);
  bindProfileImagePickers();
  bindProviderLogos();
  bindApiKeyPickers();
  bindSettingsActions(signal);
  const closeMoreActions = bindItemDetailLayout(signal);
  const openResumeOptions = bindResumeOptions(signal);
  document.querySelectorAll<HTMLImageElement>('[data-item-cover-image], [data-episode-still]').forEach((image) => {
    const showFallback = () => {
      image.hidden = true;
      image.parentElement?.classList.remove('has-image');
      const fallback = image.parentElement?.querySelector<HTMLElement>('.item-cover-fallback');
      if (fallback) fallback.hidden = false;
    };
    image.addEventListener('error', showFallback);
    if (image.complete && image.naturalWidth === 0) showFallback();
  });
  document.querySelector<HTMLButtonElement>('[data-retry-page]')?.addEventListener('click', () => { void retryPodcastAction().then(retried => { if (!retried) return refreshData(); }); });
  document.querySelector<HTMLFormElement>('#mobile-library-form')?.addEventListener('submit', (event) => event.preventDefault());
  document.querySelector<HTMLSelectElement>('#mobile-library-picker')?.addEventListener('change', (event) => {
    navigateTo((event.currentTarget as HTMLSelectElement).value);
  });
  document.querySelector<HTMLButtonElement>('[data-mobile-settings]')?.addEventListener('click', () => navigateTo('/settings'));
  document.querySelector<HTMLButtonElement>('[data-mobile-sign-out]')?.addEventListener('click', () => {
    document.querySelector<HTMLButtonElement>('[data-sign-out]')?.click();
  });
  document.querySelector<HTMLButtonElement>('#toggle-library-filters')?.addEventListener('click', () => {
    state.browseFiltersOpen = !state.browseFiltersOpen;
    render(true);
    requestAnimationFrame(() => document.querySelector<HTMLButtonElement>('#toggle-library-filters')?.focus({ preventScroll: true }));
  });
  document.querySelector<HTMLFormElement>('#library-browse-form')?.addEventListener('change', (event) => {
    const form = event.currentTarget as HTMLFormElement;
    const values = Object.fromEntries(new FormData(form));
    state.browsePreferences = normalizeBrowsePreferences(values);
    state.libraryPage = 0;
    saveBrowsePreferences(state.browsePreferences, state.bootstrap?.current_user?.id, activeLibraryId());
    render(true);
  });
  document.querySelector<HTMLFormElement>('#library-browse-form')?.addEventListener('submit', (event) => event.preventDefault());
  document.querySelector<HTMLButtonElement>('#reset-library-filters')?.addEventListener('click', () => {
    state.browsePreferences = { ...DEFAULT_BROWSE_PREFERENCES, density: state.browsePreferences.density };
    state.libraryPage = 0;
    saveBrowsePreferences(state.browsePreferences, state.bootstrap?.current_user?.id, activeLibraryId());
    render(true);
  });
  document.querySelectorAll<HTMLButtonElement>('[data-library-page]').forEach((button) => {
    button.addEventListener('click', () => {
      const lastPage = Math.max(0, Math.ceil(visibleLibraryItems().length / LIBRARY_PAGE_SIZE) - 1);
      state.libraryPage = Math.max(0, Math.min(lastPage, Number(button.dataset.libraryPage)));
      render(true);
      requestAnimationFrame(() => {
        const card = document.querySelector<HTMLElement>('.library-item-grid .media-card');
        card?.focus({ preventScroll: true });
        card?.scrollIntoView({ block: 'nearest' });
      });
    });
  });
  document.querySelector<HTMLDetailsElement>('#item-management')?.addEventListener('toggle', (event) => {
    state.itemManagementOpen = (event.currentTarget as HTMLDetailsElement).open;
    if (state.itemManagementOpen && !state.selectedItem?.podcast) void loadItemSources(render);
  });
  document.querySelectorAll<HTMLButtonElement>('[data-item-sources-page]').forEach((button) => {
    button.addEventListener('click', () => { void loadItemSources(render, Number(button.dataset.itemSourcesPage)); });
  });
  document.querySelector<HTMLFormElement>('#welcome-user-form')?.addEventListener('submit', async (event) => {
    event.preventDefault();
    const form = event.currentTarget as HTMLFormElement | null;
    if (!form) {
      return;
    }

    try {
      const request = await createUserRequest(form, true);
      setAuthFormBusy(form, true);
      await createUser(request);
      const token = await loginUser({ username: request.username, password: request.password });
      setStoredAuthToken(token.token);
      await refreshData(false);
    } catch (error) {
      setAuthFormBusy(form, false);
      state.error = error instanceof Error ? error.message : 'Failed to create the first user.';
      render();
    }
  });

  document.querySelector<HTMLFormElement>('#login-form')?.addEventListener('submit', async (event) => {
    event.preventDefault();
    const form = event.currentTarget as HTMLFormElement | null;
    if (!form) {
      return;
    }

    const formData = new FormData(form);
    const request: LoginRequest = {
      username: formDataString(formData.get('username')).trim(),
      password: formDataString(formData.get('password')),
    };

    try {
      setAuthFormBusy(form, true);
      const token = await loginUser(request);
      setStoredAuthToken(token.token);
      await refreshData(false);
    } catch (error) {
      setAuthFormBusy(form, false);
      clearStoredAuthToken();
      state.error = error instanceof Error ? error.message : 'Failed to sign in.';
      render();
    }
  });

  document.querySelector<HTMLButtonElement>('[data-sign-out]')?.addEventListener('click', async () => {
    stopMusic();
    if (state.isPlayerOpen || state.activeTvPlayback) closeActivePlaybackSession();
    try {
      await logoutUser();
    } catch (error) {
      state.error = error instanceof Error ? error.message : 'Failed to sign out.';
      render();
      return;
    }
    clearBrowseSessions();
    state.bootstrap = state.bootstrap ? { ...state.bootstrap, current_user: undefined } : undefined;
    void refreshData();
  });

  document.querySelector<HTMLElement>('[data-nav-home]')?.addEventListener('click', () => {
    navigateTo('/');
  });

  document.querySelectorAll<HTMLElement>('[data-nav-library-id]').forEach((button) => {
    button.addEventListener('click', () => {
      const libraryId = Number(button.dataset.navLibraryId);
      if (!Number.isFinite(libraryId)) {
        return;
      }

      navigateTo(`/libraries/${libraryId}`);
    });
  });

  document.querySelector<HTMLElement>('[data-nav-settings]')?.addEventListener('click', () => {
    navigateTo('/settings');
  });

  document.querySelectorAll<HTMLElement>('[data-settings-section-path]').forEach((button) => {
    button.addEventListener('click', () => {
      const path = button.dataset.settingsSectionPath;
      if (path) {
        navigateTo(path);
      }
    });
  });
  document.querySelectorAll<HTMLAnchorElement>('.settings-nav-link').forEach((link) => {
    link.addEventListener('click', (event) => {
      if (event.button === 0 && !event.ctrlKey && !event.metaKey && !event.shiftKey && !event.altKey) {
        event.preventDefault();
        navigateTo(link.pathname);
      }
    });
  });

  document.querySelectorAll<HTMLButtonElement>('[data-provider-settings]').forEach((button) => {
    button.addEventListener('click', () => {
      const providerId = button.dataset.providerSettings;
      const providerHash = providerId ? `#provider-${providerId}` : '';
      navigateTo(`/settings/providers${providerHash}`);
    });
  });

  bindProviderOrdering();

  document.querySelector<HTMLFormElement>('#search-form')?.addEventListener('submit', (event) => {
    event.preventDefault();
    const input = document.querySelector<HTMLInputElement>('#search-input');
    state.searchQuery = input?.value ?? '';
    state.showFullSearchResults = Boolean(state.searchQuery.trim());
    void refreshHomeSearch(context);
  });

  document.querySelector<HTMLInputElement>('#search-input')?.addEventListener('input', (event) => {
    liveSearchGeneration += 1;
    const input = event.currentTarget as HTMLInputElement;
    state.searchQuery = input.value;
    state.searchLoading = Boolean(input.value.trim());
    state.showFullSearchResults = false;
    if (pendingLiveSearchHandle !== undefined) {
      globalThis.clearTimeout(pendingLiveSearchHandle);
    }
    pendingLiveSearchHandle = globalThis.setTimeout(() => {
      pendingLiveSearchHandle = undefined;
      void refreshHomeSearch(context);
    }, 250);
    render(true);
  });

  document.querySelector<HTMLButtonElement>('[data-clear-search]')?.addEventListener('click', () => {
    clearHomeSearch();
    render();
    document.querySelector<HTMLInputElement>('#search-input')?.focus();
  });

  document.querySelector<HTMLButtonElement>('#refresh-active-library-metadata')?.addEventListener('click', async () => {
    const button = document.querySelector<HTMLButtonElement>('#refresh-active-library-metadata');
    const library = activeLibrary();
    if (!library || libraryHasActiveMetadataRefresh(library.id)) {
      return;
    }

    try {
      setButtonBusy(button, true);
      const refreshedLibrary = await mutateMetadata(() => refreshLibraryMetadata(library.id));
      state.libraries = state.libraries.map((entry) => entry.id === refreshedLibrary.id ? refreshedLibrary : entry);
      await refreshPendingMetadataData();
      schedulePendingMetadataRefresh(true);
    } catch (error) {
      state.error = error instanceof Error ? error.message : 'Failed to refresh library metadata.';
      render();
    }
  });

  document.querySelectorAll<HTMLButtonElement>('[data-home-tab]').forEach((button) => {
    button.addEventListener('click', () => {
      const nextTab = button.dataset.homeTab as HomeBrowseTab | undefined;
      if (!nextTab) {
        return;
      }

      if (state.route.page === 'browse-detail') {
        state.homeTab = nextTab;
        state.browseFilter = undefined;
        state.homePreviewItemId = undefined;
        state.homePreviewCollectionId = undefined;
        clearHomeSearch();
        const nextPath = homeBrowsePath();
        globalThis.history.pushState({}, '', nextPath);
        state.route = parseRoute();
        void refreshData();
        return;
      }

      const clearedSearch = clearHomeSearch();
      if (state.homeTab === nextTab) {
        if (clearedSearch) {
          render();
        }
        return;
      }

      state.homeTab = nextTab;
      state.browseFilter = undefined;
      state.homePreviewItemId = undefined;
      state.homePreviewCollectionId = undefined;
      render();
    });
  });

  document.querySelectorAll<HTMLButtonElement>('[data-category-filter]').forEach((button) => {
    button.addEventListener('click', () => {
      const genre = button.dataset.categoryFilter;
      if (!genre) {
        return;
      }

      const category = categorySummaries().find((entry) => entry.genre === genre);
      if (!category) {
        return;
      }

      navigateTo(browseDetailPath('category', category.genre));
    });
  });

  document.querySelectorAll<HTMLButtonElement>('[data-playlist-filter]').forEach((button) => {
    button.addEventListener('click', () => {
      const playlistName = button.dataset.playlistFilter;
      if (!playlistName) {
        return;
      }

      navigateTo(browseDetailPath('playlist', playlistName));
    });
  });

  document.querySelectorAll<HTMLButtonElement>('[data-collection-filter]').forEach((button) => {
    button.addEventListener('click', () => {
      const collectionId = button.dataset.collectionFilter;
      if (!collectionId) {
        return;
      }

      const collection = collectionSummaries().find((entry) => entry.id === collectionId);
      const searchCollection = searchResultCollections().find((entry) => entry.id === collectionId);
      if (!collection && !searchCollection) {
        return;
      }

      if (collection) {
        navigateTo(browseDetailPath('collection', collection.id));
      } else {
        navigateTo(`/items/collections/${encodeURIComponent(searchCollection!.id)}`);
      }
    });
  });

  document.querySelector<HTMLButtonElement>('#clear-browse-filter')?.addEventListener('click', () => {
    state.browseFilter = undefined;
    navigateTo(typeof activeLibraryId() === 'number' ? `/libraries/${activeLibraryId()}` : '/');
  });

  document.querySelectorAll<HTMLElement>('[data-item-id]').forEach(bindItemNavigationElement);

  document.querySelectorAll<HTMLElement>('[data-person-id]').forEach((button) => {
    button.addEventListener('click', () => {
      const personId = Number(button.dataset.personId);
      if (!Number.isFinite(personId)) {
        return;
      }

      navigateTo(`/people/${personId}`);
    });
  });
  bindPersonCreditTrays();

  document.querySelectorAll<HTMLButtonElement>('[data-toggle-text]').forEach((button) => {
    button.addEventListener('click', () => {
      const key = button.dataset.toggleText;
      if (!key) {
        return;
      }
      if (state.expandedTextKeys.has(key)) {
        state.expandedTextKeys.delete(key);
      } else {
        state.expandedTextKeys.add(key);
      }
      render();
    });
  });

  document.querySelector<HTMLButtonElement>('#back-to-library')?.addEventListener('click', () => {
    if (state.route.page === 'person') {
      globalThis.history.back();
      return;
    }

    navigateTo(backNavigationTarget().path);
  });

  document.querySelector<HTMLInputElement>('#metadata-search-artist')?.addEventListener('input', (event) => {
    state.metadataSearchArtist = (event.target as HTMLInputElement).value;
  });
  document.querySelector<HTMLFormElement>('#metadata-search-form')?.addEventListener('submit', async (event) => {
    event.preventDefault();
    if (!state.selectedItem) {
      return;
    }

    const input = document.querySelector<HTMLInputElement>('#metadata-search-input');
    const yearInput = document.querySelector<HTMLInputElement>('#metadata-search-year');
    const artistInput = document.querySelector<HTMLInputElement>('#metadata-search-artist');
    const languageInput = document.querySelector<HTMLInputElement>('#metadata-search-language');
    state.metadataSearchQuery = input?.value.trim() || selectedItemDefaultMetadataTitle();
    state.metadataSearchYear = yearInput ? yearInput.value.trim() || selectedItemDefaultMetadataYear() : '';
    state.metadataSearchArtist = artistInput?.value.trim();
    state.metadataSearchLanguage = languageInput?.value.trim() || defaultMetadataSearchLanguage();
    state.metadataSearchProviders = Array.from(
      document.querySelectorAll<HTMLInputElement>('input[name="metadataSearchProvider"]:checked'),
    ).map((provider) => provider.value);
    try {
      const submitButton = document.querySelector<HTMLButtonElement>('#metadata-search-form button[type="submit"]');
      setButtonBusy(submitButton, true);
      state.metadataSearchResults = await searchItemMetadata(state.selectedItem.id, {
        query: state.metadataSearchQuery,
        artist: state.metadataSearchArtist,
        providers: state.metadataSearchProviders,
        year: state.metadataSearchYear,
        language: state.metadataSearchLanguage,
      });
      render();
    } catch (error) {
      state.error = error instanceof Error ? error.message : 'Failed to search metadata.';
      render();
    }
  });

  document.querySelector<HTMLButtonElement>('[data-audiobook-versions]')?.addEventListener('click', async () => {
    const itemId = state.selectedItem?.id;
    if (itemId === undefined) return;
    state.audiobookRecordingChoices = { itemId, loading: true, choices: [] };
    render();
    try {
      const choices = await getAudiobookRecordings(itemId);
      if (state.selectedItem?.id === itemId) state.audiobookRecordingChoices = { itemId, loading: false, choices };
    } catch (error) {
      if (state.selectedItem?.id === itemId) state.audiobookRecordingChoices = { itemId, loading: false, choices: [], error: error instanceof Error ? error.message : 'Could not load versions.' };
    }
    render();
  });

  document.querySelectorAll<HTMLElement>('[data-link-metadata]').forEach((button) => {
    button.addEventListener('click', async () => {
      const encoded = button.dataset.linkMetadata;
      if (!encoded || !state.selectedItem) {
        return;
      }

      const [itemId, providerId, externalId, mediaType] = encoded.split(':');
      try {
        await mutateMetadata(() => linkItemMetadata(Number(itemId), {
          provider_id: providerId,
          external_id: externalId,
          media_type: mediaType,
        }));
        clearBrowseSessions();
        state.metadataSearchResults = [];
        state.audiobookRecordingChoices = undefined;
        await refreshPendingMetadataData();
        state.hasDeferredAutoRefreshRender = false;
        render();
      } catch (error) {
        state.error = error instanceof Error ? error.message : 'Failed to link metadata.';
        render();
      }
    });
  });

  document.querySelector<HTMLButtonElement>('#unlink-item-metadata')?.addEventListener('click', async () => {
    if (!state.selectedItem) {
      return;
    }

    const button = document.querySelector<HTMLButtonElement>('#unlink-item-metadata');
    try {
      setButtonBusy(button, true);
      const itemId = state.selectedItem.id;
      await mutateMetadata(() => unlinkItemMetadata(itemId));
      clearBrowseSessions();
      state.metadataSearchResults = [];
      await refreshPendingMetadataData();
      state.hasDeferredAutoRefreshRender = false;
      render();
    } catch (error) {
      state.error = error instanceof Error ? error.message : 'Failed to unlink item metadata.';
      render();
    } finally {
      setButtonBusy(button, false);
    }
  });

  document.querySelector<HTMLButtonElement>('#refresh-item-metadata')?.addEventListener('click', async () => {
    if (!state.selectedItem) {
      return;
    }

    try {
      const itemId = state.selectedItem.id;
      await mutateMetadata(() => refreshItemMetadata(itemId));
      await refreshPendingMetadataData();
      schedulePendingMetadataRefresh();
    } catch (error) {
      state.error = error instanceof Error ? error.message : 'Failed to refresh item metadata.';
      render();
    }
  });

  const trailerButton = document.querySelector<HTMLButtonElement>('#play-item-trailer');
  if (trailerButton) {
    let trailerHoldHandle: number | undefined;
    let suppressNextTrailerClick = false;

    const clearTrailerHoldHandle = (): void => {
      if (trailerHoldHandle !== undefined) {
        globalThis.clearTimeout(trailerHoldHandle);
        trailerHoldHandle = undefined;
      }
    };

    const openTrailerChooser = (): void => {
      if (currentTrailerOptions().length <= 1) {
        return;
      }

      suppressNextTrailerClick = true;
      state.isTrailerMenuOpen = true;
      render();
      requestAnimationFrame(() => {
        const option = document.querySelector<HTMLButtonElement>('[data-play-trailer-index]');
        option?.scrollIntoView({ block: 'center' });
        option?.focus({ preventScroll: true });
      });
    };

    document.querySelector<HTMLButtonElement>('#choose-item-trailer')?.addEventListener('click', () => {
      closeMoreActions();
      openTrailerChooser();
      // The chooser button doesn't generate a click on the main trailer button.
      suppressNextTrailerClick = false;
    });

    trailerButton.addEventListener('click', () => {
      if (suppressNextTrailerClick) {
        suppressNextTrailerClick = false;
        return;
      }

      openTrailer(currentTrailerOptions()[0]);
    });
    trailerButton.addEventListener('contextmenu', (event) => {
      if (currentTrailerOptions().length <= 1) {
        return;
      }

      event.preventDefault();
      clearTrailerHoldHandle();
      openTrailerChooser();
    });
    trailerButton.addEventListener('mousedown', () => {
      clearTrailerHoldHandle();
      if (currentTrailerOptions().length <= 1) {
        return;
      }

      trailerHoldHandle = globalThis.setTimeout(() => {
        trailerHoldHandle = undefined;
        openTrailerChooser();
      }, 450);
    });
    trailerButton.addEventListener('mouseup', clearTrailerHoldHandle);
    trailerButton.addEventListener('mouseleave', clearTrailerHoldHandle);
    trailerButton.addEventListener('touchstart', () => {
      clearTrailerHoldHandle();
      if (currentTrailerOptions().length <= 1) {
        return;
      }

      trailerHoldHandle = globalThis.setTimeout(() => {
        trailerHoldHandle = undefined;
        openTrailerChooser();
      }, 500);
    }, { passive: true });
    trailerButton.addEventListener('touchend', clearTrailerHoldHandle);
    trailerButton.addEventListener('touchcancel', clearTrailerHoldHandle);
  }

  document.querySelector<HTMLButtonElement>('#close-trailer-picker')?.addEventListener('click', () => {
    state.isTrailerMenuOpen = false;
    render();
    requestAnimationFrame(() => document.querySelector<HTMLButtonElement>('#play-item-trailer')?.focus({ preventScroll: true }));
  });

  document.querySelectorAll<HTMLButtonElement>('[data-play-trailer-index]').forEach((button) => {
    button.addEventListener('click', () => {
      const trailerIndex = Number(button.dataset.playTrailerIndex);
      if (!Number.isFinite(trailerIndex)) {
        return;
      }

      openTrailer(currentTrailerOptions()[trailerIndex]);
    });
  });

  document.querySelectorAll<HTMLButtonElement>('[data-play-extra-index]').forEach((button) => {
    button.addEventListener('click', () => {
      const extraIndex = Number(button.dataset.playExtraIndex);
      if (!Number.isFinite(extraIndex)) {
        return;
      }

      const extra = selectedItemExtras()[extraIndex];
      if (extra) {
        openVideoOverlay(mediaExtraToTrailerOption(extra));
      }
    });
  });

  document.querySelector<HTMLButtonElement>('#close-trailer')?.addEventListener('click', () => {
    closeTrailerPlayer();
  });

  document.querySelector<HTMLButtonElement>('#play-youtube-theme-song')?.addEventListener('click', () => {
    const target = currentThemeSongYouTubeTarget();
    if (target) {
      playYouTubeThemeSong(target.videoId);
    }
  });

  document.querySelectorAll<HTMLButtonElement>('[data-play-music-album-id], [data-play-music-track-id]').forEach((button) => {
    button.addEventListener('click', async () => {
      try {
        if (button.dataset.playMusicAlbumId && state.selectedItem) await startMusicPlayback(state.selectedItem);
        else if (!await toggleCurrentMusic(Number(button.dataset.playMusicTrackId))) await startPlaybackForItemId(Number(button.dataset.playMusicTrackId), 0);
      } catch (error) {
        state.error = error instanceof Error ? error.message : 'Failed to start music playback.';
        render();
      }
    });
  });
  document.querySelector<HTMLButtonElement>('[data-toggle-music-album]')?.addEventListener('click', () => {
    void toggleCurrentMusic().catch((error) => { state.error = error instanceof Error ? error.message : 'Unable to change playback.'; render(); });
  });
  // Delegate radio resumes so cards appended by shelf pagination work too.
  document.addEventListener('click', (event) => {
    const button = (event.target as Element).closest<HTMLElement>('[data-resume-radio]');
    if (button) void resumeMusicQueue(Number(button.dataset.resumeRadio), browseDetailPath('playlist', button.dataset.resumeRadio!)).catch((error) => {
      state.error = error instanceof Error ? error.message : 'Unable to resume radio.'; render();
    });
  }, { signal });
  document.querySelectorAll<HTMLButtonElement>('[data-music-enqueue], [data-music-radio], [data-play-playlist], [data-play-playlist-track], [data-delete-playlist]').forEach((button) => {
    button.addEventListener('click', async () => {
      button.closest<HTMLElement>('[popover]')?.hidePopover();
      try {
        if (button.dataset.musicEnqueue) await queueMusicItem(Number(button.dataset.musicEnqueue), button.dataset.musicNext === 'true');
        else if (button.dataset.musicRadio) await startMusicRadio(Number(button.dataset.musicRadio));
        else if (button.dataset.playPlaylist) await playMusicPlaylist(Number(button.dataset.playPlaylist));
        else if (button.dataset.playPlaylistTrack) await playMusicPlaylist(Number(button.dataset.playPlaylistTrack), Number(button.dataset.playlistIndex));
        else if (button.dataset.deletePlaylist) { await deletePlaylist(Number(button.dataset.deletePlaylist)); state.homeTab = 'playlists'; navigateTo(homeBrowsePath()); }
      } catch (error) { state.error = error instanceof Error ? error.message : 'Music operation failed.'; render(); }
    });
  });
  bindSongMenus(signal);

  document.querySelectorAll<HTMLButtonElement>('[data-play-selected-item-start-ms]').forEach((button) => {
    const playFrom = async (startMs: number): Promise<void> => {
      if (!state.selectedItem) {
        return;
      }

      try {
        await startPlayback(state.selectedItem, Number.isFinite(startMs) ? startMs : 0);
      } catch (error) {
        state.error = error instanceof Error ? error.message : 'Failed to start playback session.';
        state.isPlayerOpen = false;
        state.activePlaybackItem = undefined;
        render();
      }
    };
    if (button.dataset.resumeHold !== undefined) bindResumeHold(button, openResumeOptions, signal);
    button.addEventListener('click', () => { void playFrom(Number(button.dataset.playSelectedItemStartMs)); });
  });

  document.querySelectorAll<HTMLButtonElement>('[data-playback-target-item-id]').forEach((button) => {
    const itemId = Number(button.dataset.playbackTargetItemId);
    const playFrom = async (startMs: number): Promise<void> => {
      if (!Number.isFinite(itemId)) {
        return;
      }

      try {
        await startPlaybackForItemId(itemId, Number.isFinite(startMs) ? startMs : 0);
      } catch (error) {
        state.error = error instanceof Error ? error.message : 'Failed to start playback session.';
        state.isPlayerOpen = false;
        state.activePlaybackItem = undefined;
        render();
      }
    };
    if (button.dataset.resumeHold !== undefined) bindResumeHold(button, openResumeOptions, signal);
    button.addEventListener('click', () => { void playFrom(Number(button.dataset.playbackTargetStartMs)); });
  });

  document.querySelector<HTMLButtonElement>('#close-player')?.addEventListener('click', () => {
    dismissPlayer();
  }, {signal});
  document.querySelector<HTMLButtonElement>('#expand-tv-player')?.addEventListener('click', () => {
    state.isPlayerOpen = true; render();
  }, {signal});

  document.querySelector<HTMLFormElement>('#settings-form')?.addEventListener('submit', async (event) => {
    event.preventDefault();
    const form = event.currentTarget as HTMLFormElement | null;
    if (!form) {
      return;
    }

    const owner = state.bootstrap?.current_user?.id;
    const formData = new FormData(form);
    const nextSettings = buildSettingsFromForm(formData);
    const personal = buildPodcastUserSettingsFromForm(formData);
    if (!nextSettings && !personal) return;
    try {
      if (nextSettings) {
        const response = await updateSettings(nextSettings);
        if (state.bootstrap?.current_user?.id !== owner) return;
        state.settingsResponse = response;
      }
      if (personal) await setPodcastUserSettings(personal);
      if (state.bootstrap?.current_user?.id !== owner) return;
      clearBrowseSessions();
      state.home = undefined; state.libraryItems = []; state.homePreviewItemId = undefined;
      form.reset();
      await refreshData();
    } catch (error) {
      if (state.bootstrap?.current_user?.id !== owner) return;
      state.error = error instanceof Error ? error.message : 'Failed to save settings.';
      render();
    }
  });

  document.querySelector<HTMLButtonElement>('#go-home-from-settings')?.addEventListener('click', () => {
    navigateTo('/');
  });

  document.querySelector<HTMLButtonElement>('#clear-metadata-cache')?.addEventListener('click', async () => {
    const confirmed = globalThis.confirm('Clear cached provider metadata responses? The next metadata refresh will fetch fresh data from providers.');
    if (!confirmed) {
      return;
    }
    try {
      const button = document.querySelector<HTMLButtonElement>('#clear-metadata-cache');
      setButtonBusy(button, true);
      const response = await clearMetadataCache();
      state.error = `Cleared ${response.removed_files} metadata cache file${response.removed_files === 1 ? '' : 's'}.`;
      render();
    } catch (error) {
      state.error = error instanceof Error ? error.message : 'Failed to clear metadata cache.';
      render();
    }
  });

  document.querySelectorAll<HTMLButtonElement>('[data-run-scheduled-task]').forEach((button) => {
    button.addEventListener('click', async () => {
      const taskId = button.dataset.runScheduledTask as ScheduledTaskId | undefined;
      if (!taskId) {
        return;
      }

      try {
        setButtonBusy(button, true);
        const response = await runScheduledTask(taskId);
        state.error = response.message;
        await refreshData(false);
      } catch (error) {
        state.error = error instanceof Error ? error.message : 'Failed to start scheduled task.';
        render();
      }
    });
  });

  document.querySelector<HTMLFormElement>('#metadata-dashboard-filter-form')?.addEventListener('submit', (event) => {
    event.preventDefault();
    const form = event.currentTarget as HTMLFormElement | null;
    if (!form) {
      return;
    }

    const formData = new FormData(form);
    state.metadataDashboardFilters = {
      libraryId: formDataString(formData.get('dashboard_library_id')).trim(),
      itemType: formDataString(formData.get('dashboard_item_type')).trim(),
      refreshState: formDataString(formData.get('dashboard_refresh_state')).trim(),
      search: formDataString(formData.get('dashboard_search')).trim(),
    };
    const root = document.querySelector<HTMLElement>('#metadata-dashboard-panel-root');
    if (!root) {
      render();
      return;
    }
    setElementHtml(root, renderMetadataDashboard());
    createIcons({ icons });
    bindEvents(context);
  });

  document.querySelector<HTMLButtonElement>('#clear-metadata-dashboard-filters')?.addEventListener('click', () => {
    state.metadataDashboardFilters = {
      libraryId: '',
      itemType: '',
      refreshState: '',
      search: '',
    };
    const root = document.querySelector<HTMLElement>('#metadata-dashboard-panel-root');
    if (!root) {
      render();
      return;
    }
    setElementHtml(root, renderMetadataDashboard());
    createIcons({ icons });
    bindEvents(context);
  });

  document.querySelector<HTMLFormElement>('#log-filter-form')?.addEventListener('submit', async (event) => {
    event.preventDefault();
    if (logsAreLoading()) {
      return;
    }
    const form = event.currentTarget as HTMLFormElement | null;
    if (!form) {
      return;
    }

    const formData = new FormData(form);
    state.logFilters = {
      level: formDataString(formData.get('log_level')).trim().toUpperCase(),
      module: formDataString(formData.get('log_module')).trim(),
      search: formDataString(formData.get('log_search')).trim(),
      since: formDataString(formData.get('log_since')).trim(),
      until: formDataString(formData.get('log_until')).trim(),
    };
    await refreshLogsView(context);
  });

  document.querySelector<HTMLButtonElement>('#clear-log-filters')?.addEventListener('click', async () => {
    state.logFilters = {
      level: '',
      module: '',
      search: '',
      since: '',
      until: '',
    };
    document.querySelector<HTMLFormElement>('#log-filter-form')?.querySelectorAll<HTMLInputElement | HTMLSelectElement>('input, select').forEach((control) => {
      control.value = '';
    });
    await refreshLogsView(context);
  });

  document.querySelector<HTMLButtonElement>('#refresh-log-viewer')?.addEventListener('click', async () => {
    await refreshLogsView(context);
  });
  document.querySelector<HTMLElement>('#log-viewer-panel-root')?.addEventListener('click', (event) => {
    const button = event.target instanceof Element ? event.target.closest<HTMLButtonElement>('[data-log-page]') : null;
    if (!button || button.disabled) {
      return;
    }
    const page = Number(button.dataset.logPage);
    const lastPage = Math.max(0, Math.ceil((state.logsResponse?.entries.length ?? 0) / LOG_PAGE_SIZE) - 1);
    state.logPage = Math.min(Math.max(0, page), lastPage);
    updateLogResults(context);
    document.querySelector<HTMLElement>('#log-results')?.scrollIntoView({ block: 'nearest' });
  });

  document.querySelectorAll<HTMLButtonElement>('[data-shelf-scroll]').forEach((button) => {
    button.addEventListener('click', () => {
      const target = parseShelfScrollTarget(button.dataset.shelfScroll);
      const row = target
        ? document.querySelector<HTMLElement>(`[data-shelf-row="${CSS.escape(target.shelfId)}"]`)
        : undefined;
      if (!row || !target) {
        return;
      }
      row.scrollBy({ left: target.direction * Math.max(320, row.clientWidth * 0.8), behavior: 'smooth' });
      globalThis.setTimeout(() => {
        appendLazyShelfItemsIfNeeded(row);
        updateShelfScrollControls(row);
      }, 220);
    });
  });

  document.querySelectorAll<HTMLElement>('[data-lazy-shelf-id]').forEach((row) => {
    row.addEventListener('scroll', () => {
      appendLazyShelfItemsIfNeeded(row);
      updateShelfScrollControls(row);
    }, { passive: true });
  });
  document.querySelectorAll<HTMLElement>('[data-shelf-row]:not([data-lazy-shelf-id])').forEach((row) => {
    row.addEventListener('scroll', () => updateShelfScrollControls(row), { passive: true });
  });
  globalThis.requestAnimationFrame(refreshShelfScrollControls);
  if (!shelfScrollResizeBound) {
    globalThis.addEventListener('resize', refreshShelfScrollControls, { passive: true });
    shelfScrollResizeBound = true;
  }

  document.querySelectorAll<HTMLElement>('[data-preview-item-id]').forEach(bindPreviewItemElement);
  document.querySelectorAll<HTMLElement>('[data-preview-collection-id]').forEach(bindPreviewCollectionElement);
  bindPodcastDirectoryPreviews(document, () => refreshHomeFeatureElement(undefined));
  bindHomeFeatureAction();
  document.querySelector<HTMLButtonElement>('#scan-active-library')?.addEventListener('click', async () => {
    const button = document.querySelector<HTMLButtonElement>('#scan-active-library');
    const library = activeLibrary();
    if (!library) {
      return;
    }

    try {
      setButtonBusy(button, true);
      const scannedLibrary = await scanLibrary(library.id);
      state.libraries = state.libraries.map((entry) => entry.id === scannedLibrary.id ? scannedLibrary : entry);
      await refreshData(false);
    } catch (error) {
      state.error = error instanceof Error ? error.message : 'Failed to scan library.';
      render();
    }
  });

  document.querySelectorAll<HTMLFormElement>('[data-update-user-id]').forEach((form) => {
    form.addEventListener('submit', async (event) => {
      event.preventDefault();
      const userId = Number(form.dataset.updateUserId);
      if (!Number.isFinite(userId)) {
        return;
      }

      try {
        const formData = new FormData(form);
        const profileImageUpload = await readProfileImageUpload(formData);
        const removeProfileImage = formData.get('remove_profile_image') === 'on';
        const request: UpdateUserRequest = {
          username: formDataString(formData.get('username')).trim(),
          admin: formData.get('admin') === 'on',
          birthday: formDataString(formData.get('birthday')).trim() || undefined,
          profile_image_upload: profileImageUpload,
          remove_profile_image: removeProfileImage,
          preferred_metadata_languages: parseMetadataLanguageInput(formData.get('preferred_metadata_languages')),
        };
        const updatedUser = await updateUser(userId, request);
        form.reset();
        state.users = state.users.map((user) => (user.id === updatedUser.id ? updatedUser : user));
        if (state.bootstrap?.current_user?.id === updatedUser.id) {
          state.bootstrap.current_user = updatedUser;
        }
        render();
      } catch (error) {
        state.error = error instanceof Error ? error.message : 'Failed to update the user.';
        render();
      }
    });
  });

  document.querySelector<HTMLFormElement>('#create-user-form')?.addEventListener('submit', async (event) => {
    event.preventDefault();
    const form = event.currentTarget as HTMLFormElement | null;
    if (!form) {
      return;
    }

    try {
      const request = await createUserRequest(form);
      await createUser(request);
      form.reset();
      state.settingsCreation = undefined;
      state.users = await getUsers();
      render();
      document.querySelector<HTMLButtonElement>('[data-settings-create="users"]')?.focus({ preventScroll: true });
    } catch (error) {
      state.error = error instanceof Error ? error.message : 'Failed to create the user.';
      render();
    }
  });

  document.querySelectorAll<HTMLElement>('[data-remove-library-index]').forEach((button) => {
    button.addEventListener('click', async () => {
      const libraryIndex = Number(button.dataset.removeLibraryIndex);
      if (!Number.isFinite(libraryIndex)) {
        return;
      }

      const confirmed = globalThis.confirm('Remove this library from settings? This only removes the configuration, not the media files on disk.');
      if (!confirmed) {
        return;
      }

      try {
        state.settingsResponse = await deleteLibrary(libraryIndex);
        await refreshData();
      } catch (error) {
        state.error = error instanceof Error ? error.message : 'Failed to remove library.';
        render();
      }
    });
  });

  document.querySelectorAll<HTMLElement>('[data-refresh-library-id]').forEach((button) => {
    button.addEventListener('click', async () => {
      const libraryId = Number(button.dataset.refreshLibraryId);
      if (!Number.isFinite(libraryId)) {
        return;
      }

      try {
        const library = await mutateMetadata(() => refreshLibraryMetadata(libraryId));
        state.libraries = state.libraries.map((entry) => entry.id === library.id ? library : entry);
        await refreshPendingMetadataData();
        schedulePendingMetadataRefresh(true);
      } catch (error) {
        state.error = error instanceof Error ? error.message : 'Failed to refresh library metadata.';
        render();
      }
    });
  });

  document.querySelector<HTMLFormElement>('#add-library-form')?.addEventListener('submit', async (event) => {
    event.preventDefault();
    const form = event.currentTarget as HTMLFormElement | null;
    if (!form) {
      return;
    }

    const formData = new FormData(form);
    const paths = parsePathsInput(formData.get('library_paths'));
    const library: MediaLibrarySettings = {
      name: formDataString(formData.get('library_name')),
      path: paths[0] ?? '',
      paths,
      recursive: formData.get('library_recursive') === 'on',
      exclude_from_home: formData.get('library_exclude_from_home') === 'on',
      kind: formDataString(formData.get('library_kind'), 'movies'),
      use_embedded_music_tags: formData.get('library_use_embedded_music_tags') === 'on',
      scanner: formDataString(formData.get('library_scanner'), 'auto'),
      metadata_providers: formDataStrings(formData.getAll('library_metadata_provider')),
      metadata_language_mode: formDataString(formData.get('library_metadata_language_mode'), 'auto') === 'manual' ? 'manual' : 'auto',
      metadata_languages: normalizedMetadataLanguages(formDataStrings(formData.getAll('library_metadata_language'))),
      allowed_user_ids: formData.getAll('library_allowed_user')
        .map(Number)
        .filter((value) => Number.isFinite(value) && value > 0),
    };

    try {
      state.settingsResponse = await addLibrary(library);
      form.reset();
      state.settingsCreation = undefined;
      await refreshData();
      document.querySelector<HTMLButtonElement>('[data-settings-create="libraries"]')?.focus({ preventScroll: true });
    } catch (error) {
      state.error = error instanceof Error ? error.message : 'Failed to add library.';
      render();
    }
  });
  document.querySelectorAll<HTMLElement>('[data-scan-library-id]').forEach((button) => {
    button.addEventListener('click', async () => {
      const libraryId = Number(button.dataset.scanLibraryId);
      if (!Number.isFinite(libraryId)) {
        return;
      }

      try {
        const scannedLibrary = await scanLibrary(libraryId);
        state.libraries = state.libraries.map((entry) => entry.id === scannedLibrary.id ? scannedLibrary : entry);
        await refreshData(false);
      } catch (error) {
        state.error = error instanceof Error ? error.message : 'Failed to scan library.';
        render();
      }
    });
  });
  document.querySelectorAll<HTMLButtonElement>('[data-delete-missing-library-id]').forEach((button) => {
    button.addEventListener('click', async () => {
      const libraryId = Number(button.dataset.deleteMissingLibraryId);
      if (!Number.isFinite(libraryId)) {
        return;
      }

      const library = state.libraries.find((entry) => entry.id === libraryId);
      const missingItems = library?.missing_items ?? 0;
      const missingFiles = library?.missing_files ?? 0;
      if (!globalThis.confirm(`Delete ${missingItems} missing item${missingItems === 1 ? '' : 's'} and ${missingFiles} missing file${missingFiles === 1 ? '' : 's'} from this library?`)) {
        return;
      }

      button.disabled = true;
      try {
        const cleanup = await deleteMissingItems(libraryId);
        state.libraries = state.libraries.map((entry) => entry.id === cleanup.library.id ? cleanup.library : entry);
        await refreshData(false);
      } catch (error) {
        state.error = error instanceof Error ? error.message : 'Failed to delete missing items.';
        render();
      }
    });
  });

  const addLibraryKindSelect = document.querySelector<HTMLSelectElement>('#add-library-form select[name="library_kind"]');
  addLibraryKindSelect?.addEventListener('change', () => syncAddLibraryProviderOptions());
  syncAddLibraryProviderOptions();
  document.querySelectorAll<HTMLSelectElement>('select[name^="existing_library_kind_"]').forEach((select) => {
    select.addEventListener('change', () => select.closest('.settings-library-card')?.querySelectorAll<HTMLElement>('[data-music-tag-option]').forEach((option) => { option.hidden = select.value !== 'music'; }));
  });
  document.querySelectorAll<HTMLElement>('.metadata-provider-list').forEach(syncProviderDependencyOptions);

  bindPlayerProgress();
  bindTrailerPlayer();
}



/** Show a fallback while a provider logo loads or is unavailable. */
function bindProviderLogos(): void {
  document.querySelectorAll<HTMLImageElement>('[data-provider-logo]').forEach((image) => {
    const sync = () => image.parentElement?.classList.toggle('is-loaded', image.complete && image.naturalWidth > 0);
    image.addEventListener('load', sync);
    image.addEventListener('error', sync);
    sync();
  });
}

/** Bind attached API-key buttons; clearing stays reversible until settings are saved. */
function bindApiKeyPickers(): void {
  document.querySelectorAll<HTMLElement>('.api-key-picker').forEach((picker) => {
    const input = picker.querySelector<HTMLInputElement>('input[type="password"]')!;
    const removal = picker.querySelector<HTMLInputElement>('[data-api-key-removal]')!;
    const button = picker.querySelector<HTMLButtonElement>('[data-clear-api-key]')!;
    const status = picker.closest('.api-key-field')?.querySelector<HTMLElement>('[data-api-key-status]');
    const configured = picker.dataset.apiKeyConfigured === 'true';
    const savedPlaceholder = configured ? 'Saved' : '';
    const label = picker.getAttribute('aria-label') ?? 'API key';
    const sync = () => {
      input.placeholder = removal.checked ? 'Will be cleared on save' : savedPlaceholder;
      button.disabled = !configured && !input.value && !removal.checked;
      button.setAttribute('aria-pressed', String(removal.checked));
      button.setAttribute('aria-label', removal.checked ? `Undo clearing ${label}` : `Clear ${label}`);
      button.title = removal.checked ? 'Undo API key clearing' : 'Clear API key on save';
      if (status) status.hidden = !removal.checked;
    };
    input.addEventListener('input', () => {
      removal.checked = false;
      sync();
    });
    button.addEventListener('click', () => {
      input.value = '';
      removal.checked = configured && !removal.checked;
      sync();
    });
    sync();
  });
}

function syncAddLibraryProviderOptions(): void {
  const form = document.querySelector<HTMLFormElement>('#add-library-form');
  const kind = form?.querySelector<HTMLSelectElement>('select[name="library_kind"]')?.value;
  if (!form || !kind) {
    return;
  }

  form.querySelectorAll<HTMLElement>('[data-music-tag-option]').forEach((option) => { option.hidden = kind !== 'music'; });
  form.querySelectorAll<HTMLInputElement>('input[name="library_metadata_provider"]').forEach((input) => {
    const supportedKinds = input.dataset.providerKinds?.split(',') ?? [];
    const supported = supportedKinds.includes(kind);
    const option = input.closest<HTMLElement>('.metadata-provider-option');
    input.disabled = !supported;
    if (!supported) {
      input.checked = false;
    }
    option?.classList.toggle('is-hidden', !supported);
  });

  const visibleCheckedProvider = form.querySelector<HTMLInputElement>(
    'input[name="library_metadata_provider"]:not(:disabled):checked',
  );
  if (!visibleCheckedProvider) {
    const firstVisibleProvider = form.querySelector<HTMLInputElement>('input[name="library_metadata_provider"]:not(:disabled)');
    if (firstVisibleProvider) {
      firstVisibleProvider.checked = true;
    }
  }
  form.querySelectorAll<HTMLElement>('.metadata-provider-list').forEach(syncProviderDependencyOptions);
}
import { loadItemSources } from './itemSources';
