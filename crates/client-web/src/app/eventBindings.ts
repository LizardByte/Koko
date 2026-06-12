/** Binds DOM events for each rendered app tree and owns transient UI timers. */
import { createIcons, icons } from 'lucide';
import { HOME_SHELF_CHUNK_SIZE } from './constants';
import { currentLogFilterRequest, libraryHasActiveMetadataRefresh } from './activities';
import { readProfileImageUpload } from './auth';
import { renderLogViewer, renderMetadataDashboard } from './dashboardView';
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
import type { HomeBrowseTab } from './types';
import { browseDetailPath, homeBrowsePath, renderHomeFeature, renderItemCard } from './homeView';
import {
  bindPersonCreditTrays,
  defaultMetadataSearchLanguage,
  selectedItemDefaultMetadataTitle,
  selectedItemDefaultMetadataYear,
  selectedItemExtras,
} from './itemPersonView';
import { buildSettingsFromForm } from './settingsView';
import { setButtonBusy } from './ui';
import {
  addLibrary,
  clearMetadataCache,
  clearStoredAuthToken,
  createUser,
  deleteLibrary,
  deleteMissingItems,
  getItemMetadata,
  getLogs,
  getUsers,
  linkItemMetadata,
  loginUser,
  refreshItemMetadata,
  refreshLibraryMetadata,
  runScheduledTask,
  scanLibrary,
  searchItemMetadata,
  setStoredAuthToken,
  updateSettings,
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

function clearHomeSearch(): boolean {
  const hadSearch = Boolean(state.searchQuery) || state.searchResults.length > 0 || state.showFullSearchResults;
  if (pendingLiveSearchHandle !== undefined) {
    globalThis.clearTimeout(pendingLiveSearchHandle);
    pendingLiveSearchHandle = undefined;
  }
  state.searchQuery = '';
  state.searchResults = [];
  state.showFullSearchResults = false;
  return hadSearch;
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



async function refreshLogsView(context: AppEventBindingContext): Promise<void> {
  const { render, setElementHtml } = context;
  if (state.route.page !== 'settings') {
    return;
  }

  try {
    state.logsResponse = await getLogs(currentLogFilterRequest());
    state.error = undefined;
  } catch (error) {
    state.error = error instanceof Error ? error.message : 'Failed to load logs.';
  } finally {
    const root = document.querySelector<HTMLElement>('#log-viewer-panel-root');
    if (!root) {
      render();
      return;
    }
    setElementHtml(root, renderLogViewer());
    createIcons({ icons });
    bindEvents(context);
  }
}



function updatePageBackdrop(backdropUrl: string | undefined): void {
  const appShell = document.querySelector<HTMLElement>('.app-shell');
  const pageBackdrop = document.querySelector<HTMLElement>('.page-backdrop');
  if (backdropUrl) {
    appShell?.classList.add('has-page-backdrop');
    if (pageBackdrop) {
      pageBackdrop.style.setProperty('--page-backdrop-image', `url('${backdropUrl.replace(/'/g, "\\'")}')`);
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
    root.outerHTML = renderHomeFeature();
    createIcons({ icons });
    bindHomeFeatureAction();
  }
  updatePageBackdrop(backdropUrl);
}



function activatePreviewItem(itemId: number): void {
  if (state.route.page === 'browse-detail' || !Number.isFinite(itemId) || state.homePreviewItemId === itemId) {
    return;
  }
  state.homePreviewItemId = itemId;
  state.homePreviewCollectionId = undefined;
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
  element.addEventListener('mouseenter', updatePreview);
  element.addEventListener('focus', updatePreview);
}



function bindPreviewCollectionElement(element: HTMLElement): void {
  if (element.dataset.boundPreviewCollection === 'true') {
    return;
  }
  element.dataset.boundPreviewCollection = 'true';
  const updatePreview = (): void => {
    activatePreviewCollection(element.dataset.previewCollectionId);
  };
  element.addEventListener('mouseenter', updatePreview);
  element.addEventListener('focus', updatePreview);
}



function bindMediaCardInteractions(root: ParentNode): void {
  root.querySelectorAll<HTMLElement>('[data-item-id]').forEach(bindItemNavigationElement);
  root.querySelectorAll<HTMLElement>('[data-preview-item-id]').forEach(bindPreviewItemElement);
  root.querySelectorAll<HTMLElement>('[data-preview-collection-id]').forEach(bindPreviewCollectionElement);
}



function homeShelfForRow(row: HTMLElement): MediaShelf | undefined {
  const shelfId = row.dataset.lazyShelfId;
  return shelfId ? state.home?.shelves.find((shelf) => shelf.id === shelfId) : undefined;
}



function appendLazyShelfItems(row: HTMLElement): boolean {
  const shelf = homeShelfForRow(row);
  if (!shelf) {
    return false;
  }

  const renderedCount = Number(row.dataset.lazyRenderedCount ?? row.children.length);
  if (!Number.isFinite(renderedCount) || renderedCount >= shelf.items.length) {
    row.dataset.lazyComplete = 'true';
    return false;
  }

  const nextCount = Math.min(shelf.items.length, renderedCount + HOME_SHELF_CHUNK_SIZE);
  row.insertAdjacentHTML('beforeend', shelf.items.slice(renderedCount, nextCount).map(renderItemCard).join(''));
  row.dataset.lazyRenderedCount = String(nextCount);
  row.dataset.lazyComplete = nextCount >= shelf.items.length ? 'true' : 'false';
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
    const optionsWithSignal = typeof options === 'boolean'
      ? { capture: options, signal }
      : { ...(options ?? {}), signal };
    originalAddEventListener.call(this, type, listener, optionsWithSignal);
  } as typeof EventTarget.prototype.addEventListener;

  try {
    bindRenderEvents(context);
  } finally {
    EventTarget.prototype.addEventListener = originalAddEventListener;
  }
}



function bindRenderEvents(context: AppEventBindingContext): void {
  const { navigateTo, refreshData, refreshPendingMetadataData, render, schedulePendingMetadataRefresh, setElementHtml } = context;
  document.querySelector<HTMLFormElement>('#welcome-user-form')?.addEventListener('submit', async (event) => {
    event.preventDefault();
    const form = event.currentTarget as HTMLFormElement | null;
    if (!form) {
      return;
    }

    try {
      const formData = new FormData(form);
      const request: CreateUserRequest = {
        username: formDataString(formData.get('username')).trim(),
        password: formDataString(formData.get('password')),
        pin: formDataString(formData.get('pin')).trim() || undefined,
        admin: true,
        birthday: formDataString(formData.get('birthday')).trim() || undefined,
        profile_image_upload: await readProfileImageUpload(formData),
        preferred_metadata_languages: parseMetadataLanguageInput(formData.get('preferred_metadata_languages')),
      };
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

  document.querySelector<HTMLButtonElement>('[data-sign-out]')?.addEventListener('click', () => {
    clearStoredAuthToken();
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

  document.querySelectorAll<HTMLButtonElement>('[data-provider-settings]').forEach((button) => {
    button.addEventListener('click', () => {
      const providerId = button.dataset.providerSettings;
      const providerHash = providerId ? `#provider-${providerId}` : '';
      navigateTo(`/settings/providers${providerHash}`);
    });
  });

  document.querySelectorAll<HTMLButtonElement>('[data-provider-move]').forEach((button) => {
    button.addEventListener('click', () => {
      const option = button.closest<HTMLElement>('.metadata-provider-option');
      const list = option?.closest<HTMLElement>('.metadata-provider-list');
      if (!option || !list) {
        return;
      }
      if (button.dataset.providerMove === 'up' && option.previousElementSibling) {
        list.insertBefore(option, option.previousElementSibling);
      }
      if (button.dataset.providerMove === 'down' && option.nextElementSibling) {
        list.insertBefore(option.nextElementSibling, option);
      }
      syncProviderDependencyOptions(list);
    });
  });

  document.querySelectorAll<HTMLInputElement>('.metadata-provider-list input[type="checkbox"]').forEach((input) => {
    input.addEventListener('change', () => {
      const list = input.closest<HTMLElement>('.metadata-provider-list');
      if (list) {
        syncProviderDependencyOptions(list);
      }
    });
  });

  document.querySelector<HTMLFormElement>('#search-form')?.addEventListener('submit', (event) => {
    event.preventDefault();
    const input = document.querySelector<HTMLInputElement>('#search-input');
    state.searchQuery = input?.value ?? '';
    state.showFullSearchResults = Boolean(state.searchQuery.trim());
    void refreshData();
  });

  document.querySelector<HTMLInputElement>('#search-input')?.addEventListener('input', (event) => {
    const input = event.currentTarget as HTMLInputElement;
    state.searchQuery = input.value;
    state.showFullSearchResults = false;
    if (pendingLiveSearchHandle !== undefined) {
      globalThis.clearTimeout(pendingLiveSearchHandle);
    }
    pendingLiveSearchHandle = globalThis.setTimeout(() => {
      pendingLiveSearchHandle = undefined;
      void refreshData(false);
    }, 250);
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
      const refreshedLibrary = await refreshLibraryMetadata(library.id);
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

  document.querySelector<HTMLFormElement>('#metadata-search-form')?.addEventListener('submit', async (event) => {
    event.preventDefault();
    if (!state.selectedItem) {
      return;
    }

    const input = document.querySelector<HTMLInputElement>('#metadata-search-input');
    const yearInput = document.querySelector<HTMLInputElement>('#metadata-search-year');
    const languageInput = document.querySelector<HTMLInputElement>('#metadata-search-language');
    state.metadataSearchQuery = input?.value.trim() || selectedItemDefaultMetadataTitle();
    state.metadataSearchYear = yearInput?.value.trim() || selectedItemDefaultMetadataYear();
    state.metadataSearchLanguage = languageInput?.value.trim() || defaultMetadataSearchLanguage();
    state.metadataSearchProviders = Array.from(
      document.querySelectorAll<HTMLInputElement>('input[name="metadataSearchProvider"]:checked'),
    ).map((provider) => provider.value);
    try {
      const submitButton = document.querySelector<HTMLButtonElement>('#metadata-search-form button[type="submit"]');
      setButtonBusy(submitButton, true);
      state.metadataSearchResults = await searchItemMetadata(state.selectedItem.id, {
        query: state.metadataSearchQuery,
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

  document.querySelectorAll<HTMLElement>('[data-link-metadata]').forEach((button) => {
    button.addEventListener('click', async () => {
      const encoded = button.dataset.linkMetadata;
      if (!encoded || !state.selectedItem) {
        return;
      }

      const [itemId, providerId, externalId, mediaType] = encoded.split(':');
      try {
        await linkItemMetadata(Number(itemId), {
          provider_id: providerId,
          external_id: externalId,
          media_type: mediaType,
        });
        state.selectedItemMetadata = await getItemMetadata(state.selectedItem.id);
        state.metadataSearchResults = [];
        await refreshPendingMetadataData();
      } catch (error) {
        state.error = error instanceof Error ? error.message : 'Failed to link metadata.';
        render();
      }
    });
  });

  document.querySelector<HTMLButtonElement>('#refresh-item-metadata')?.addEventListener('click', async () => {
    if (!state.selectedItem) {
      return;
    }

    try {
      await refreshItemMetadata(state.selectedItem.id);
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
    };

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

  document.querySelectorAll<HTMLButtonElement>('[data-play-selected-item-start-ms]').forEach((button) => button.addEventListener('click', async () => {
    if (!state.selectedItem) {
      return;
    }

    try {
      const startMs = Number(button.dataset.playSelectedItemStartMs);
      await startPlayback(state.selectedItem, Number.isFinite(startMs) ? startMs : 0);
    } catch (error) {
      state.error = error instanceof Error ? error.message : 'Failed to start playback session.';
      state.isPlayerOpen = false;
      state.activePlaybackItem = undefined;
      render();
    }
  }));

  document.querySelectorAll<HTMLButtonElement>('[data-playback-target-item-id]').forEach((button) => button.addEventListener('click', async () => {
    const itemId = Number(button.dataset.playbackTargetItemId);
    const startMs = Number(button.dataset.playbackTargetStartMs);
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
  }));

  document.querySelector<HTMLButtonElement>('#close-player')?.addEventListener('click', () => {
    closeActivePlaybackSession();
  });

  document.querySelector<HTMLFormElement>('#settings-form')?.addEventListener('submit', async (event) => {
    event.preventDefault();
    const form = event.currentTarget as HTMLFormElement | null;
    if (!form) {
      return;
    }

    const nextSettings = buildSettingsFromForm(new FormData(form));
    if (!nextSettings) {
      return;
    }

    try {
      state.settingsResponse = await updateSettings(nextSettings);
      await refreshData();
    } catch (error) {
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
    await refreshLogsView(context);
  });

  document.querySelector<HTMLButtonElement>('#refresh-log-viewer')?.addEventListener('click', async () => {
    await refreshLogsView(context);
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
      const formData = new FormData(form);
      const request: CreateUserRequest = {
        username: formDataString(formData.get('username')).trim(),
        password: formDataString(formData.get('password')),
        pin: formDataString(formData.get('pin')).trim() || undefined,
        admin: formData.get('admin') === 'on',
        birthday: formDataString(formData.get('birthday')).trim() || undefined,
        profile_image_upload: await readProfileImageUpload(formData),
        preferred_metadata_languages: parseMetadataLanguageInput(formData.get('preferred_metadata_languages')),
      };
      await createUser(request);
      form.reset();
      state.users = await getUsers();
      render();
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
        const library = await refreshLibraryMetadata(libraryId);
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
      kind: formDataString(formData.get('library_kind'), 'movies'),
      scanner: formDataString(formData.get('library_scanner'), 'auto'),
      metadata_providers: formDataStrings(formData.getAll('library_metadata_provider')),
      metadata_language_mode: formDataString(formData.get('library_metadata_language_mode'), 'auto') === 'manual' ? 'manual' : 'auto',
      metadata_languages: normalizedMetadataLanguages(formDataStrings(formData.getAll('library_metadata_language'))),
      allowed_user_ids: formData.getAll('library_allowed_user')
        .map((value) => Number(value))
        .filter((value) => Number.isFinite(value) && value > 0),
    };

    try {
      state.settingsResponse = await addLibrary(library);
      form.reset();
      await refreshData();
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
  document.querySelectorAll<HTMLElement>('.metadata-provider-list').forEach(syncProviderDependencyOptions);

  bindPlayerProgress();
  bindTrailerPlayer();
}



function syncProviderDependencyOptions(list: HTMLElement): void {
  const selectedPrimaryIds = new Set(
    Array.from(list.querySelectorAll<HTMLInputElement>('.metadata-provider-option[data-provider-role="primary"] input[type="checkbox"]:checked'))
      .map((input) => input.value),
  );
  let priority = 0;
  list.querySelectorAll<HTMLElement>('.metadata-provider-option').forEach((option) => {
    const input = option.querySelector<HTMLInputElement>('input[type="checkbox"]');
    const label = option.querySelector<HTMLElement>('.provider-option-main .muted');
    const role = option.dataset.providerRole ?? 'primary';
    if (role === 'primary') {
      priority += 1;
      if (label) {
        label.textContent = `Priority ${priority}`;
      }
      return;
    }

    const extendsProviderIds = (option.dataset.extendsProviderIds ?? '')
      .split(',')
      .map((value) => value.trim())
      .filter(Boolean);
    const available = extendsProviderIds.some((providerId) => selectedPrimaryIds.has(providerId));
    if (input) {
      input.disabled = !available;
      if (!available) {
        input.checked = false;
      }
    }
    option.classList.toggle('is-disabled', !available);
    if (label) {
      label.textContent = available ? 'Secondary' : 'Requires primary provider';
    }
  });
}



function syncAddLibraryProviderOptions(): void {
  const form = document.querySelector<HTMLFormElement>('#add-library-form');
  const kind = form?.querySelector<HTMLSelectElement>('select[name="library_kind"]')?.value;
  if (!form || !kind) {
    return;
  }

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
