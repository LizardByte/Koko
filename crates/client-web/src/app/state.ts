/** Initializes shared browser application state from routing and saved preferences. */

import { getApiMode, getStoredApiBase } from '../api';
import { defaultHomeTab, parseRoute } from './routes';
import type { AppState } from './types';
import { DEFAULT_BROWSE_PREFERENCES } from './browsePreferences';

/** Shared mutable application state for the browser client. */
export const state: AppState = {
  apiBase: getStoredApiBase(),
  apiMode: getApiMode(),
  route: parseRoute(),
  users: [],
  libraries: [],
  libraryItems: [],
  libraryItemsLoading: false,
  itemMetadataLoading: false,
  searchResults: [],
  homePreviewItemId: undefined,
  homePreviewCollectionId: undefined,
  metadataProviders: [],
  systemActivities: [],
  dashboardItems: [],
  metadataSearchResults: [],
  searchQuery: '',
  metadataSearchQuery: '',
  metadataSearchArtist: undefined,
  metadataSearchYear: '',
  metadataSearchLanguage: 'en',
  metadataSearchProviders: [],
  showFullSearchResults: false,
  homeTab: defaultHomeTab(parseRoute()),
  browsePreferences: { ...DEFAULT_BROWSE_PREFERENCES },
  browseFiltersOpen: false,
  libraryPage: 0,
  itemManagementOpen: false,
  itemMoreOpen: false,
  resumeOptionsOpen: false,
  searchLoading: false,
  isLoading: true,
  hasDeferredAutoRefreshRender: false,
  isPlayerOpen: false,
  activePlaybackItem: undefined,
  activePlaybackStartMs: 0,
  activeAudioStreamIndex: undefined,
  isAudioTrackMenuOpen: false,
  isTrailerMenuOpen: false,
  activeTrailer: undefined,
  expandedTextKeys: new Set(),
  metadataDashboardFilters: {
    libraryId: '',
    itemType: '',
    refreshState: '',
    search: '',
  },
  logFilters: {
    level: '',
    module: '',
    search: '',
    since: '',
    until: '',
  },
  logPage: 0,
};
