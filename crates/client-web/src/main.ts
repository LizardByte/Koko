import './style.css';
import kokoLogoUrl from '../../../assets/Koko.svg';
import { createIcons, icons } from 'lucide';
import {
  addLibrary,
  clearMetadataCache,
  clearStoredAuthToken,
  createUser,
  deleteLibrary,
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
  getPersonImageUrl,
  getMetadataProviders,
  getLogs,
  getPlaybackDecision,
  getSystemActivities,
  refreshLibraryMetadata,
  refreshItemMetadata,
  getSettings,
  getStoredAuthToken,
  getStoredApiBase,
  getUsers,
  linkItemMetadata,
  loginUser,
  resolveApiUrl,
  scanLibrary,
  searchItemMetadata,
  searchItems,
  setStoredAuthToken,
  updatePlaybackProgress,
  updateSettings,
  updateUser,
  type AppBootstrapResponse,
  type ApiMode,
  type BootstrapUser,
  type CreateUserRequest,
  type UpdateUserRequest,
  type MediaCollectionSummary,
  type ItemMetadataPerson,
  type ItemMetadataResponse,
  type LoginRequest,
  type MediaHome,
  type MediaItemDetail,
  type MediaItemSummary,
  type MediaLibrary,
  type MediaLibrarySettings,
  type MetadataProviderStatus,
  type MetadataProviderSettings,
  type MetadataPersonItemCredit,
  type MetadataPersonResponse,
  type MetadataSearchResult,
  type LogEntriesResponse,
  type PlaybackDecision,
  type SettingsResponse,
  type SettingsSnapshot,
  type ServerCapabilities,
  type SystemActivity,
  getWebClientProfile,
  type PlaybackSession,
  createPlaybackSession,
  deletePlaybackSession,
  getSessionStreamUrl,
} from './api';

type AppRoute =
  | { page: 'home'; libraryId?: number }
  | { page: 'browse-detail'; kind: 'category' | 'collection' | 'playlist'; key: string; libraryId?: number }
  | { page: 'item'; itemId: number }
  | { page: 'person'; personId: number }
  | { page: 'settings'; section?: SettingsSection };

type HomeBrowseTab = 'recommended' | 'library' | 'collections' | 'playlists' | 'categories';
type SettingsSection = 'general' | 'libraries' | 'providers' | 'dashboard' | 'logs';

interface BrowseFilter {
  kind: 'category' | 'collection' | 'playlist';
  label: string;
  itemIds: number[];
  overview?: string;
  artworkUrl?: string;
}

interface TrailerOption {
  title: string;
  url: string;
  label?: string;
  titleSuffix?: string;
}

type ThemeSongSource =
  | { kind: 'audio'; src: string; title: string }
  | { kind: 'youtube'; src: string; title: string; videoId: string };

interface YouTubePlayer {
  loadVideoById(videoId: string): void;
  playVideo(): void;
  pauseVideo(): void;
  seekTo(seconds: number, allowSeekAhead: boolean): void;
  getCurrentTime(): number;
  getDuration(): number;
  getPlayerState(): number;
  setVolume(volume: number): void;
  getVolume(): number;
  mute(): void;
  unMute(): void;
  isMuted(): boolean;
  setPlaybackQuality(suggestedQuality: string): void;
  destroy(): void;
}

interface YouTubeIframeApi {
  Player: new (
    elementId: string,
    options: {
      height: string;
      width: string;
      videoId?: string;
      playerVars?: Record<string, number | string>;
      events?: {
        onReady?: (event: { target: YouTubePlayer }) => void;
        onStateChange?: () => void;
        onError?: (event: { data: number }) => void;
      };
    },
  ) => YouTubePlayer;
}

declare global {
  interface Window {
    YT?: YouTubeIframeApi;
    onYouTubeIframeAPIReady?: () => void;
  }
}

interface AppState {
  apiBase: string;
  apiMode: ApiMode;
  route: AppRoute;
  bootstrap?: AppBootstrapResponse;
  users: BootstrapUser[];
  capabilities?: ServerCapabilities;
  libraries: MediaLibrary[];
  home?: MediaHome;
  libraryItems: MediaItemSummary[];
  libraryItemsLoading: boolean;
  searchResults: MediaItemSummary[];
  homePreviewItemId?: number;
  homePreviewCollectionId?: string;
  metadataProviders: MetadataProviderStatus[];
  systemActivities: SystemActivity[];
  dashboardItems: MediaItemSummary[];
  settingsResponse?: SettingsResponse;
  logsResponse?: LogEntriesResponse;
  selectedItem?: MediaItemDetail;
  selectedItemMetadata?: ItemMetadataResponse;
  selectedPerson?: MetadataPersonResponse;
  selectedPlayback?: PlaybackDecision;
  metadataSearchResults: MetadataSearchResult[];
  searchQuery: string;
  metadataSearchQuery: string;
  metadataSearchYear: string;
  metadataSearchLanguage: string;
  metadataSearchProviders: string[];
  showFullSearchResults: boolean;
  homeTab: HomeBrowseTab;
  browseFilter?: BrowseFilter;
  isLoading: boolean;
  isPlayerOpen: boolean;
  activePlaybackSession?: PlaybackSession;
  activePlaybackStartMs: number;
  activeAudioStreamIndex?: number;
  isAudioTrackMenuOpen: boolean;
  isTrailerMenuOpen: boolean;
  activeTrailer?: TrailerOption;
  expandedTextKeys: Set<string>;
  error?: string;
  hasDeferredAutoRefreshRender: boolean;
  metadataDashboardFilters: {
    libraryId: string;
    itemType: string;
    refreshState: string;
    search: string;
  };
  logFilters: {
    level: string;
    module: string;
    search: string;
    since: string;
    until: string;
  };
}

type AppIconName =
  | 'arrow-left'
  | 'arrow-right'
  | 'book'
  | 'chevron-left'
  | 'chevron-right'
  | 'clapperboard'
  | 'database-zap'
  | 'film'
  | 'folder-sync'
  | 'house'
  | 'image'
  | 'languages'
  | 'layout-grid'
  | 'link-2'
  | 'log-in'
  | 'log-out'
  | 'music'
  | 'maximize'
  | 'pause'
  | 'picture-in-picture'
  | 'play'
  | 'plus'
  | 'refresh-cw'
  | 'save'
  | 'search'
  | 'settings'
  | 'skip-back'
  | 'skip-forward'
  | 'trash-2'
  | 'tv'
  | 'triangle-alert'
  | 'user-plus'
  | 'volume-2'
  | 'volume-x'
  | 'x';

const state: AppState = {
  apiBase: getStoredApiBase(),
  apiMode: getApiMode(),
  route: parseRoute(),
  users: [],
  libraries: [],
  libraryItems: [],
  libraryItemsLoading: false,
  searchResults: [],
  homePreviewItemId: undefined,
  homePreviewCollectionId: undefined,
  metadataProviders: [],
  systemActivities: [],
  dashboardItems: [],
  metadataSearchResults: [],
  searchQuery: '',
  metadataSearchQuery: '',
  metadataSearchYear: '',
  metadataSearchLanguage: 'en',
  metadataSearchProviders: [],
  showFullSearchResults: false,
  homeTab: defaultHomeTab(parseRoute()),
  isLoading: true,
  hasDeferredAutoRefreshRender: false,
  isPlayerOpen: false,
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
};

const app = document.querySelector<HTMLDivElement>('#app');
if (!app) {
  throw new Error('Failed to find app container');
}
const appRoot = app;
const YOUTUBE_THEME_PLACEHOLDER_VIDEO_ID = 'dQw4w9WgXcQ';
const YOUTUBE_PLAYER_STATE = {
  ended: 0,
  playing: 1,
  paused: 2,
  buffering: 3,
  cued: 5,
} as const;
let pendingLibraryRefreshHandle: number | undefined;
let pendingMetadataRefreshHandle: number | undefined;
let pendingLiveSearchHandle: number | undefined;
let youtubeIframeApiPromise: Promise<YouTubeIframeApi> | undefined;
let themeSongYouTubePlayer: YouTubePlayer | undefined;
let themeSongYouTubePlayerReady: Promise<YouTubePlayer> | undefined;
let activeThemeSongYouTubeVideoId: string | undefined;
let trailerYouTubePlayer: YouTubePlayer | undefined;
let trailerYouTubePlayerReady: Promise<YouTubePlayer> | undefined;
let activeTrailerYouTubeVideoId: string | undefined;
let trailerProgressHandle: number | undefined;
let trailerVolume = 1;
let trailerMuted = false;
const activeGamepadButtons = new Set<string>();

function activeMetadataRefreshActivities(): SystemActivity[] {
  return state.systemActivities.filter((activity) => {
    return activity.category === 'metadata_refresh'
      && activity.state !== 'completed'
      && activity.state !== 'failed';
  });
}

function activeMetadataRefreshItemIds(): Set<number> {
  return new Set(activeMetadataRefreshActivities().flatMap((activity) => activity.item_ids));
}

function itemHasActiveMetadataRefresh(itemId?: number): boolean {
  return typeof itemId === 'number' && activeMetadataRefreshItemIds().has(itemId);
}

function activityProgress(activity: Pick<SystemActivity, 'completed_items' | 'total_items' | 'failed_items'>): {
  completed: number;
  total: number;
  failed: number;
  percent: number;
} {
  const total = Math.max(0, activity.total_items);
  const completed = Math.min(total, Math.max(0, activity.completed_items));
  const failed = Math.max(0, activity.failed_items);
  const percent = total > 0 ? Math.min(100, Math.max(0, (completed / total) * 100)) : 0;
  return { completed, total, failed, percent };
}

function metadataRefreshActivityProgressForLibrary(libraryId: number): {
  completed: number;
  total: number;
  failed: number;
  percent: number;
} | undefined {
  const activities = activeMetadataRefreshActivities().filter((activity) => activity.library_id === libraryId);
  if (!activities.length) {
    return undefined;
  }

  const totals = activities.reduce((summary, activity) => {
    const progress = activityProgress(activity);
    return {
      completed: summary.completed + progress.completed,
      total: summary.total + progress.total,
      failed: summary.failed + progress.failed,
    };
  }, { completed: 0, total: 0, failed: 0 });
  if (totals.total <= 0) {
    return undefined;
  }

  return {
    ...totals,
    percent: Math.min(100, Math.max(0, (totals.completed / totals.total) * 100)),
  };
}

function currentLogFilterRequest(): { level?: string; module?: string; search?: string; since?: string; until?: string; limit: number } {
  return {
    level: state.logFilters.level || undefined,
    module: state.logFilters.module || undefined,
    search: state.logFilters.search || undefined,
    since: state.logFilters.since || undefined,
    until: state.logFilters.until || undefined,
    limit: 200,
  };
}

function snapshotJson(value: unknown): string {
  return JSON.stringify(value);
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

function defaultHomeTab(_route: AppRoute): HomeBrowseTab {
  return 'recommended';
}

function parseRoute(): AppRoute {
  const normalizedPath = window.location.pathname.replace(/\/+$/, '') || '/';

  const settingsMatch = normalizedPath.match(/^\/settings(?:\/(libraries|providers|dashboard|logs))?$/);
  if (settingsMatch) {
    return { page: 'settings', section: (settingsMatch[1] as SettingsSection | undefined) ?? 'general' };
  }

  const itemMatch = normalizedPath.match(/^\/items\/(\d+)$/);
  if (itemMatch) {
    return { page: 'item', itemId: Number(itemMatch[1]) };
  }

  const personMatch = normalizedPath.match(/^\/people\/(\d+)$/);
  if (personMatch) {
    return { page: 'person', personId: Number(personMatch[1]) };
  }

  const libraryBrowseMatch = normalizedPath.match(/^\/libraries\/(\d+)\/items\/(collections|categories|playlists)\/(.+)$/);
  if (libraryBrowseMatch) {
    return {
      page: 'browse-detail',
      libraryId: Number(libraryBrowseMatch[1]),
      kind: libraryBrowseMatch[2] === 'collections'
        ? 'collection'
        : libraryBrowseMatch[2] === 'playlists'
          ? 'playlist'
          : 'category',
      key: decodeURIComponent(libraryBrowseMatch[3]),
    };
  }

  const browseMatch = normalizedPath.match(/^\/items\/(collections|categories|playlists)\/(.+)$/);
  if (browseMatch) {
    return {
      page: 'browse-detail',
      kind: browseMatch[1] === 'collections'
        ? 'collection'
        : browseMatch[1] === 'playlists'
          ? 'playlist'
          : 'category',
      key: decodeURIComponent(browseMatch[2]),
    };
  }

  const libraryMatch = normalizedPath.match(/^\/libraries\/(\d+)$/);
  if (libraryMatch) {
    return { page: 'home', libraryId: Number(libraryMatch[1]) };
  }

  return { page: 'home' };
}

function clearPendingLibraryRefresh(): void {
  if (pendingLibraryRefreshHandle !== undefined) {
    window.clearTimeout(pendingLibraryRefreshHandle);
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

  pendingLibraryRefreshHandle = window.setTimeout(() => {
    pendingLibraryRefreshHandle = undefined;
    void refreshPendingLibraryData();
  }, 1800);
}

function clearPendingMetadataRefresh(): void {
  if (pendingMetadataRefreshHandle !== undefined) {
    window.clearTimeout(pendingMetadataRefreshHandle);
    pendingMetadataRefreshHandle = undefined;
  }
}

function itemIsMetadataPending(item: Pick<MediaItemSummary, 'id' | 'metadata_refresh_state'> | undefined): boolean {
  return item?.metadata_refresh_state === 'pending' || itemHasActiveMetadataRefresh(item?.id);
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
  return [...state.libraryItems, ...state.searchResults, ...visibleShelfItems]
    .some((item) => item.metadata_refresh_state === 'pending');
}

function schedulePendingMetadataRefresh(): void {
  clearPendingMetadataRefresh();
  if (!shouldAutoRefreshMetadata()) {
    return;
  }

  pendingMetadataRefreshHandle = window.setTimeout(() => {
    pendingMetadataRefreshHandle = undefined;
    void refreshPendingMetadataData();
  }, 1500);
}

function navigateTo(path: string, replace = false): void {
  const currentPath = `${window.location.pathname}${window.location.search}`;
  if (currentPath === path) {
    state.route = parseRoute();
    render();
    return;
  }

  if (replace) {
    window.history.replaceState({}, '', path);
  } else {
    window.history.pushState({}, '', path);
  }
  state.route = parseRoute();
  if (state.route.page === 'home') {
    state.homeTab = defaultHomeTab(state.route);
    state.browseFilter = undefined;
  }
  state.isTrailerMenuOpen = false;
  void refreshData();
}

function formatTimestamp(timestamp?: number): string {
  if (!timestamp) {
    return 'Unknown';
  }

  return new Date(timestamp * 1000).toLocaleString('en-US');
}

function formatDuration(durationMs?: number): string {
  if (!durationMs) {
    return 'Unknown';
  }

  const totalSeconds = Math.floor(durationMs / 1000);
  const hours = Math.floor(totalSeconds / 3600);
  const minutes = Math.floor((totalSeconds % 3600) / 60);
  const seconds = totalSeconds % 60;

  if (hours > 0) {
    return `${hours}:${String(minutes).padStart(2, '0')}:${String(seconds).padStart(2, '0')}`;
  }

  return `${minutes}:${String(seconds).padStart(2, '0')}`;
}

function formatMediaTime(seconds: number): string {
  if (!Number.isFinite(seconds) || seconds < 0) {
    return '0:00';
  }

  return formatDuration(Math.floor(seconds * 1000));
}

function resumablePlaybackPositionMs(item: MediaItemDetail): number {
  const positionMs = item.playback_position_ms ?? 0;
  const durationMs = item.playback_duration_ms ?? item.duration_ms ?? 0;
  if (positionMs < 30_000) {
    return 0;
  }
  if (durationMs > 0 && durationMs - positionMs < 30_000) {
    return 0;
  }

  return positionMs;
}

function formatFileSize(fileSize?: number): string {
  if (!fileSize) {
    return 'Unknown';
  }

  const units = ['B', 'KB', 'MB', 'GB', 'TB'];
  let size = fileSize;
  let unitIndex = 0;
  while (size >= 1024 && unitIndex < units.length - 1) {
    size /= 1024;
    unitIndex += 1;
  }

  return `${size.toFixed(size >= 10 || unitIndex === 0 ? 0 : 1)} ${units[unitIndex]}`;
}

function formatBitRate(bitRate?: number): string {
  if (!bitRate) {
    return 'Unknown';
  }

  if (bitRate >= 1_000_000) {
    return `${(bitRate / 1_000_000).toFixed(bitRate >= 10_000_000 ? 0 : 1)} Mbps`;
  }

  if (bitRate >= 1_000) {
    return `${Math.round(bitRate / 1_000)} kbps`;
  }

  return `${bitRate} bps`;
}

function escapeHtml(value: string): string {
  return value
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;')
    .replace(/'/g, '&#39;');
}

const COLLAPSIBLE_TEXT_LENGTH = 520;
const COLLAPSIBLE_TEXT_LINE_COUNT = 6;

function renderCollapsibleText(text: string, key: string, className = 'hero-description'): string {
  const normalized = text.trim();
  const lineCount = normalized.split(/\r\n|\r|\n/).length;
  const shouldCollapse = normalized.length > COLLAPSIBLE_TEXT_LENGTH || lineCount > COLLAPSIBLE_TEXT_LINE_COUNT;
  const isExpanded = state.expandedTextKeys.has(key);
  const stateClass = shouldCollapse && !isExpanded ? 'is-collapsed' : '';
  const toggle = shouldCollapse
    ? `<button type="button" class="text-toggle-button" data-toggle-text="${escapeHtml(key)}" aria-expanded="${isExpanded ? 'true' : 'false'}">${isExpanded ? 'show less' : '... see more'}</button>`
    : '';

  return `
    <div class="collapsible-text ${className} ${stateClass}" data-collapsible-text="${escapeHtml(key)}">${escapeHtml(normalized)}</div>
    ${toggle}
  `;
}

function extractYouTubeVideoId(url: string): string | undefined {
  const normalizedUrl = url.trim();
  if (!normalizedUrl) {
    return undefined;
  }

  const videoIdPattern = /^[A-Za-z0-9_-]{11}$/;
  if (videoIdPattern.test(normalizedUrl)) {
    return normalizedUrl;
  }

  let parseTarget = normalizedUrl;
  if (
    normalizedUrl.startsWith('//youtube.com/')
    || normalizedUrl.startsWith('//www.youtube.com/')
    || normalizedUrl.startsWith('//youtu.be/')
    || normalizedUrl.startsWith('//youtube-nocookie.com/')
    || normalizedUrl.startsWith('//www.youtube-nocookie.com/')
  ) {
    parseTarget = `https:${normalizedUrl}`;
  } else if (
    normalizedUrl.startsWith('youtube.com/')
    || normalizedUrl.startsWith('www.youtube.com/')
    || normalizedUrl.startsWith('youtu.be/')
    || normalizedUrl.startsWith('youtube-nocookie.com/')
    || normalizedUrl.startsWith('www.youtube-nocookie.com/')
  ) {
    parseTarget = `https://${normalizedUrl}`;
  }

  try {
    const parsed = new URL(parseTarget);
    const host = parsed.hostname.toLowerCase().replace(/^www\./, '');
    if (host === 'youtu.be') {
      const videoId = parsed.pathname.split('/').filter(Boolean)[0];
      return videoIdPattern.test(videoId ?? '') ? videoId : undefined;
    }

    const isYouTubeHost = host === 'youtube.com'
      || host.endsWith('.youtube.com')
      || host === 'youtube-nocookie.com'
      || host.endsWith('.youtube-nocookie.com');
    if (isYouTubeHost) {
      if (parsed.pathname.startsWith('/watch')) {
        const videoId = parsed.searchParams.get('v')?.trim();
        return videoIdPattern.test(videoId ?? '') ? videoId : undefined;
      }

      const [kind, videoId] = parsed.pathname.split('/').filter(Boolean);
      if (['embed', 'shorts', 'live'].includes(kind ?? '')) {
        return videoIdPattern.test(videoId ?? '') ? videoId : undefined;
      }
    }
  } catch {
    return undefined;
  }

  return undefined;
}

function buildYouTubeWatchUrl(url: string): string | undefined {
  const videoId = extractYouTubeVideoId(url);
  return videoId ? `https://www.youtube.com/watch?v=${videoId}` : undefined;
}

function currentTrailerOptions(): TrailerOption[] {
  if (!state.selectedItem?.trailer_url) {
    return [];
  }

  return [{
    title: state.selectedItem.trailer_title?.trim() || 'Trailer',
    url: state.selectedItem.trailer_url,
  }];
}

function currentThemeSongTarget(): { title: string; url: string } | undefined {
  const route = state.route;
  if (route.page === 'browse-detail' && route.kind === 'collection') {
    const collection = collectionSummaries().find((entry) => entry.id === route.key);
    return collection?.theme_song_url
      ? { title: collection.name, url: collection.theme_song_url }
      : undefined;
  }

  if (route.page !== 'item' || !state.selectedItem?.theme_song_url) {
    return undefined;
  }

  return {
    title: state.selectedItem.display_title,
    url: state.selectedItem.theme_song_url,
  };
}

function currentThemeSongYouTubeTarget(): { title: string; url: string; videoId: string } | undefined {
  const target = currentThemeSongTarget();
  const videoId = target ? extractYouTubeVideoId(target.url) : undefined;
  if (!target || !videoId) {
    return undefined;
  }

  return {
    title: target.title,
    url: target.url,
    videoId,
  };
}

function openVideoOverlay(option: TrailerOption | undefined): void {
  if (!option) {
    return;
  }

  destroyTrailerYouTubePlayer();
  state.activeTrailer = option;
  state.isTrailerMenuOpen = false;
  render();
}

function openTrailer(option: TrailerOption | undefined): void {
  openVideoOverlay(option);
}

function closeTrailerPlayer(): void {
  state.activeTrailer = undefined;
  destroyTrailerYouTubePlayer();
  render();
}

function libraryRefreshProgress(library: MediaLibrary): { completed: number; total: number; percent: number; failed: number } | undefined {
  const activityProgress = metadataRefreshActivityProgressForLibrary(library.id);
  if (activityProgress) {
    return activityProgress;
  }

  if (library.metadata_refresh_total <= 0 || library.metadata_refresh_pending <= 0) {
    return undefined;
  }

  const completed = Math.max(0, library.metadata_refresh_completed);
  const percent = Math.min(100, Math.max(0, (completed / library.metadata_refresh_total) * 100));
  return {
    completed,
    total: library.metadata_refresh_total,
    percent,
    failed: library.metadata_refresh_failed,
  };
}

function activeLibraryPendingRefreshCount(libraryId: number): number {
  return activeMetadataRefreshActivities()
    .filter((activity) => activity.library_id === libraryId)
    .reduce((count, activity) => count + Math.max(0, activity.total_items - activity.completed_items), 0);
}

function metadataDashboardRefreshState(item: MediaItemSummary): 'pending' | 'stalled' | 'error' | 'fresh' | 'unmatched' {
  if (itemIsMetadataPending(item)) {
    return itemHasActiveMetadataRefresh(item.id) ? 'pending' : 'stalled';
  }

  if (item.metadata_refresh_state === 'error') {
    return 'error';
  }

  if (item.metadata_refresh_state === 'fresh' || item.has_metadata) {
    return 'fresh';
  }

  return 'unmatched';
}

function metadataDashboardRefreshLabel(item: MediaItemSummary): string {
  switch (metadataDashboardRefreshState(item)) {
    case 'pending':
      return 'Refreshing';
    case 'stalled':
      return 'Pending without worker';
    case 'error':
      return 'Failed';
    case 'fresh':
      return 'Up to date';
    default:
      return 'Not linked';
  }
}

function filteredMetadataDashboardItems(): MediaItemSummary[] {
  const libraryFilter = state.metadataDashboardFilters.libraryId;
  const itemTypeFilter = state.metadataDashboardFilters.itemType;
  const refreshStateFilter = state.metadataDashboardFilters.refreshState;
  const searchFilter = state.metadataDashboardFilters.search.trim().toLowerCase();

  const rank = (item: MediaItemSummary): number => {
    switch (metadataDashboardRefreshState(item)) {
      case 'error':
        return 0;
      case 'stalled':
        return 1;
      case 'pending':
        return 2;
      case 'unmatched':
        return 3;
      default:
        return 4;
    }
  };

  return [...state.dashboardItems]
    .filter((item) => {
      const matchesLibrary = libraryFilter ? String(item.library_id) === libraryFilter : true;
      const matchesItemType = itemTypeFilter ? item.item_type === itemTypeFilter : true;
      const matchesRefreshState = refreshStateFilter ? metadataDashboardRefreshState(item) === refreshStateFilter : true;
      const matchesSearch = searchFilter
        ? `${item.display_title} ${item.relative_path} ${item.metadata_refresh_error ?? ''}`.toLowerCase().includes(searchFilter)
        : true;
      return matchesLibrary && matchesItemType && matchesRefreshState && matchesSearch;
    })
    .sort((left, right) => {
      return rank(left) - rank(right)
        || left.library_id - right.library_id
        || left.display_title.localeCompare(right.display_title)
        || left.relative_path.localeCompare(right.relative_path);
    });
}

function metadataDashboardSummary(items: MediaItemSummary[]): {
  failed: number;
  pending: number;
  stalled: number;
  unmatched: number;
} {
  return items.reduce((summary, item) => {
    switch (metadataDashboardRefreshState(item)) {
      case 'error':
        summary.failed += 1;
        break;
      case 'pending':
        summary.pending += 1;
        break;
      case 'stalled':
        summary.stalled += 1;
        break;
      case 'unmatched':
        summary.unmatched += 1;
        break;
      default:
        break;
    }
    return summary;
  }, {
    failed: 0,
    pending: 0,
    stalled: 0,
    unmatched: 0,
  });
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

function parsePathsInput(value: FormDataEntryValue | null | undefined): string[] {
  return String(value ?? '')
    .split(/\r?\n/)
    .map((entry) => entry.trim())
    .filter(Boolean);
}

function joinPaths(paths: string[]): string {
  return paths.join('\n');
}

function activeLibraryId(): number | undefined {
  if (state.route.page === 'home' || state.route.page === 'browse-detail') {
    return state.route.libraryId;
  }

  return state.selectedItem?.library_id;
}

function activeLibrary(): MediaLibrary | undefined {
  return state.libraries.find((library) => library.id === activeLibraryId());
}

function setButtonBusy(button: HTMLButtonElement | null | undefined, busy: boolean): void {
  if (!button) {
    return;
  }
  button.disabled = busy;
  button.classList.toggle('is-busy', busy);
  button.setAttribute('aria-busy', busy ? 'true' : 'false');
}

function activeLibrarySettings(): MediaLibrarySettings | undefined {
  const library = activeLibrary();
  if (!library || !state.settingsResponse) {
    return undefined;
  }

  return state.settingsResponse.settings.media.libraries.find((settings) => {
    const paths = [settings.path, ...settings.paths].map((path) => path.trim()).filter(Boolean);
    return settings.name === library.name
      || paths.includes(library.path)
      || library.paths.some((path) => paths.includes(path));
  });
}

function persistedLibraryForSettings(library: MediaLibrarySettings): MediaLibrary | undefined {
  const configuredPaths = [library.path, ...library.paths]
    .map((path) => path.trim())
    .filter(Boolean);
  return state.libraries.find((candidate) => {
    return configuredPaths.includes(candidate.path)
      || candidate.paths.some((path) => configuredPaths.includes(path));
  });
}

function selectedLibraryIcon(kind?: string): AppIconName {
  switch (kind) {
    case 'mixed':
      return 'layout-grid';
    case 'movies':
      return 'clapperboard';
    case 'shows':
      return 'tv';
    case 'music':
      return 'music';
    case 'photos':
      return 'image';
    case 'books':
      return 'book';
    case 'home_videos':
      return 'film';
    default:
      return 'layout-grid';
  }
}

function renderIcon(iconName: AppIconName, className = 'rail-icon'): string {
  return `<span class="${className}"><i data-lucide="${iconName}"></i></span>`;
}

function renderButtonContent(label: string, iconName?: AppIconName, iconPosition: 'start' | 'end' = 'start'): string {
  if (!iconName) {
    return escapeHtml(label);
  }

  return `
    <span class="button-content${iconPosition === 'end' ? ' icon-end' : ''}">
      ${renderIcon(iconName, 'button-icon')}
      <span>${escapeHtml(label)}</span>
    </span>
  `;
}

function isRailCollapsed(): boolean {
  return state.route.page === 'item';
}

function currentUser(): BootstrapUser | undefined {
  return state.bootstrap?.current_user;
}

function requiresSetup(): boolean {
  return state.bootstrap?.has_users === false;
}

function requiresLogin(): boolean {
  return state.bootstrap?.has_users === true && !currentUser();
}

function canManageUsers(): boolean {
  return currentUser()?.admin ?? false;
}

function renderAuthShell(title: string, description: string, content: string): string {
  return `
    <div class="auth-shell">
      <section class="auth-panel panel">
        <div class="auth-header">
          <div class="brand-mark logo-brand-mark"><img class="brand-logo" src="${escapeHtml(kokoLogoUrl)}" alt="" /></div>
          <div>
            <h1>Koko</h1>
            <p class="muted">${escapeHtml(description)}</p>
          </div>
        </div>
        <div class="auth-copy">
          <h2>${escapeHtml(title)}</h2>
        </div>
        ${state.error ? `<section class="error-panel auth-error-panel">${escapeHtml(state.error)}</section>` : ''}
        ${content}
      </section>
    </div>
  `;
}

function renderWelcomeScreen(): string {
  return renderAuthShell(
    'Create the first admin user',
    'Koko needs one administrator account before the media library can be used.',
    `
      <form id="welcome-user-form" class="auth-form">
        <label>Username<input name="username" autocomplete="username" required /></label>
        <label>Password<input name="password" type="password" autocomplete="new-password" required /></label>
        <label>Optional PIN<input name="pin" inputmode="numeric" pattern="[0-9]{4,6}" placeholder="1234" /></label>
        <label>Birthday<input name="birthday" type="date" /></label>
        <label>Profile image URL<input name="profile_image_url" type="url" placeholder="https://example.com/avatar.jpg" /></label>
        <button type="submit">${renderButtonContent('Create admin account', 'user-plus')}</button>
      </form>
    `,
  );
}

function renderLoginScreen(): string {
  return renderAuthShell(
    'Sign in',
    'Sign in with a Koko account to browse media and keep watch progress per user.',
    `
      <form id="login-form" class="auth-form">
        <label>Username<input name="username" autocomplete="username" required /></label>
        <label>Password<input name="password" type="password" autocomplete="current-password" required /></label>
        <button type="submit">${renderButtonContent('Sign in', 'log-in')}</button>
      </form>
    `,
  );
}

function renderUserManagement(): string {
  if (!canManageUsers()) {
    return '';
  }

  return `
    <section class="settings-form user-management-form">
      <div class="section-heading">
        <h3>Users</h3>
      </div>
      <div class="user-list">
        ${state.users.length
          ? state.users.map((user) => `
              <form class="provider-row user-edit-row" data-update-user-id="${user.id}">
                <div class="user-edit-fields">
                  <label>Username<input name="username" value="${escapeHtml(user.username)}" required /></label>
                  <label>Birthday<input name="birthday" type="date" value="${escapeHtml(user.birthday ?? '')}" /></label>
                  <label>Profile image URL<input name="profile_image_url" type="url" value="${escapeHtml(user.profile_image_url ?? '')}" placeholder="https://example.com/avatar.jpg" /></label>
                  <label>Metadata languages<input name="preferred_metadata_languages" value="${escapeHtml((user.preferred_metadata_languages ?? ['en-US']).join(', '))}" placeholder="en-US, es-ES" /></label>
                  <label class="checkbox-inline"><input name="admin" type="checkbox" ${user.admin ? 'checked' : ''} /> Administrator</label>
                </div>
                <div class="provider-tags">
                  <span class="tag ${user.admin ? 'success' : ''}">${user.admin ? 'Admin' : 'User'}</span>
                  <button type="submit" class="secondary-button">${renderButtonContent('Save', 'save')}</button>
                </div>
              </form>
            `).join('')
          : '<div class="empty-state tight">No users found.</div>'}
      </div>
    </section>

    <form id="create-user-form" class="settings-form user-management-form">
      <section>
        <div class="section-heading">
          <h3>Add user</h3>
        </div>
        <label>Username<input name="username" autocomplete="off" required /></label>
        <label>Password<input name="password" type="password" autocomplete="new-password" required /></label>
        <label>Optional PIN<input name="pin" inputmode="numeric" pattern="[0-9]{4,6}" placeholder="1234" /></label>
        <label>Birthday<input name="birthday" type="date" /></label>
        <label>Profile image URL<input name="profile_image_url" type="url" placeholder="https://example.com/avatar.jpg" /></label>
        <label>Metadata languages<input name="preferred_metadata_languages" value="en-US" placeholder="en-US, es-ES" /></label>
        <label class="checkbox-inline"><input name="admin" type="checkbox" /> Administrator</label>
        <button type="submit">${renderButtonContent('Create user', 'user-plus')}</button>
      </section>
    </form>
  `;
}

function humanizeItemType(itemType: string): string {
  switch (itemType) {
    case 'show':
      return 'Show';
    case 'season':
      return 'Season';
    case 'episode':
      return 'Episode';
    case 'movie':
      return 'Movie';
    case 'track':
      return 'Track';
    case 'photo':
      return 'Photo';
    case 'book':
      return 'Book';
    default:
      return itemType.replace(/_/g, ' ').replace(/\b\w/g, (character) => character.toUpperCase());
  }
}

function canManuallyLinkMetadata(item?: MediaItemSummary): boolean {
  return item?.item_type === 'movie' || item?.item_type === 'show';
}

function backNavigationTarget(): { label: string; path: string } {
  const hierarchy = state.selectedItem?.hierarchy ?? [];
  const parent = hierarchy[hierarchy.length - 1];
  if (parent) {
    return {
      label: `Back to ${humanizeItemType(parent.item_type).toLowerCase()}`,
      path: `/items/${parent.id}`,
    };
  }

  const libraryId = state.selectedItem?.library_id;
  return {
    label: 'Back to library',
    path: typeof libraryId === 'number' ? `/libraries/${libraryId}` : '/',
  };
}

function formatChildCount(item: MediaItemSummary): string {
  if (!item.child_count) {
    return formatDuration(item.duration_ms);
  }

  if (item.item_type === 'show') {
    return `${item.child_count} season${item.child_count === 1 ? '' : 's'}`;
  }

  if (item.item_type === 'season') {
    return `${item.child_count} episode${item.child_count === 1 ? '' : 's'}`;
  }

  return `${item.child_count} item${item.child_count === 1 ? '' : 's'}`;
}

function libraryStatusLabel(status: string): string {
  switch (status) {
    case 'never_scanned':
      return 'Pending first scan';
    case 'available':
      return 'Ready';
    case 'missing_path':
      return 'Missing path';
    case 'not_directory':
      return 'Invalid folder';
    case 'unreadable':
      return 'Unreadable';
    case 'empty_path':
      return 'No folder';
    default:
      return status.replace(/_/g, ' ');
  }
}

function topLevelLibraryItems(): MediaItemSummary[] {
  return state.libraryItems.filter((item) => item.parent_id == null);
}

function rootItemById(): Map<number, MediaItemSummary> {
  return new Map(topLevelLibraryItems().map((item) => [item.id, item]));
}

function mediaItemsById(): Map<number, MediaItemSummary> {
  return new Map(state.libraryItems.map((item) => [item.id, item]));
}

function homePreviewItemsById(): Map<number, MediaItemSummary> {
  const items = [
    ...state.libraryItems,
    ...(state.home?.shelves ?? []).flatMap((shelf) => shelf.items),
    ...state.searchResults,
  ];

  return new Map(items.map((item) => [item.id, item]));
}

function rootAncestorForItem(item: MediaItemSummary, itemsById: Map<number, MediaItemSummary>): MediaItemSummary {
  let current = item;

  while (typeof current.parent_id === 'number') {
    const parent = itemsById.get(current.parent_id);
    if (!parent) {
      break;
    }
    current = parent;
  }

  return current;
}

function showPreviewItemForHighlight(item: MediaItemSummary): MediaItemSummary {
  if (item.item_type !== 'season' && item.item_type !== 'episode') {
    return item;
  }

  const hierarchyShow = item.hierarchy?.find((ancestor) => ancestor.item_type === 'show');
  if (hierarchyShow) {
    return hierarchyShow;
  }

  const itemsById = homePreviewItemsById();
  let current = item;
  while (typeof current.parent_id === 'number') {
    const parent = itemsById.get(current.parent_id);
    if (!parent) {
      break;
    }
    if (parent.item_type === 'show') {
      return parent;
    }
    current = parent;
  }

  return item;
}

function categorySummaries(): Array<{ genre: string; count: number; items: MediaItemSummary[] }> {
  const itemsById = mediaItemsById();
  const rootsById = rootItemById();
  const categories = new Map<string, Map<number, MediaItemSummary>>();

  state.libraryItems.forEach((item) => {
    if (!item.genres.length) {
      return;
    }

    const rootItem = rootAncestorForItem(item, itemsById);
    const root = rootsById.get(rootItem.id) ?? rootItem;
    item.genres.forEach((genre) => {
      const normalizedGenre = genre.trim();
      if (!normalizedGenre) {
        return;
      }

      if (!categories.has(normalizedGenre)) {
        categories.set(normalizedGenre, new Map());
      }
      categories.get(normalizedGenre)!.set(root.id, root);
    });
  });

  return [...categories.entries()]
    .map(([genre, items]) => ({ genre, count: items.size, items: [...items.values()] }))
    .sort((left, right) => right.count - left.count || left.genre.localeCompare(right.genre));
}

function collectionSummaries(): MediaCollectionSummary[] {
  return state.home?.collections ?? [];
}

function collectionForRoute(): MediaCollectionSummary | undefined {
  const route = state.route;
  if (route.page !== 'browse-detail' || route.kind !== 'collection') {
    return undefined;
  }

  return collectionSummaries().find((entry) => entry.id === route.key);
}

function itemsForCollection(collection: MediaCollectionSummary): MediaItemSummary[] {
  const allowedIds = new Set(collection.item_ids);
  return topLevelLibraryItems().filter((item) => allowedIds.has(item.id));
}

function selectedItemRoot(): MediaItemSummary | undefined {
  if (!state.selectedItem) {
    return undefined;
  }

  return state.selectedItem.hierarchy[0] ?? state.selectedItem;
}

function selectedItemCollectionRails(): Array<{ collection: MediaCollectionSummary; items: MediaItemSummary[] }> {
  const root = selectedItemRoot();
  if (!root) {
    return [];
  }

  return collectionSummaries()
    .filter((collection) => collection.item_ids.includes(root.id))
    .map((collection) => ({
      collection,
      items: itemsForCollection(collection).filter((item) => item.id !== root.id),
    }))
    .filter((rail) => rail.items.length > 0);
}

function categoryForRoute(): { genre: string; count: number; items: MediaItemSummary[] } | undefined {
  const route = state.route;
  if (route.page !== 'browse-detail' || route.kind !== 'category') {
    return undefined;
  }

  return categorySummaries().find((entry) => entry.genre === route.key);
}

function browseItemsForRoute(): MediaItemSummary[] {
  const route = state.route;
  if (route.page !== 'browse-detail') {
    return [];
  }

  if (route.kind === 'collection') {
    const collection = collectionForRoute();
    return collection ? itemsForCollection(collection) : [];
  }

  if (route.kind === 'category') {
    return categoryForRoute()?.items ?? [];
  }

  return [];
}

function filteredTopLevelLibraryItems(): MediaItemSummary[] {
  const items = topLevelLibraryItems();
  if (!state.browseFilter) {
    return items;
  }

  const allowedIds = new Set(state.browseFilter.itemIds);
  return items.filter((item) => allowedIds.has(item.id));
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
      searchQuery ? searchItems(searchQuery, libraryId) : Promise.resolve([]),
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

function browseDetailPath(kind: BrowseFilter['kind'], key: string): string {
  const segment = kind === 'collection' ? 'collections' : kind === 'playlist' ? 'playlists' : 'categories';
  const encodedKey = encodeURIComponent(key);
  return typeof activeLibraryId() === 'number'
    ? `/libraries/${activeLibraryId()}/items/${segment}/${encodedKey}`
    : `/items/${segment}/${encodedKey}`;
}

function homeBrowsePath(): string {
  const libraryId = activeLibraryId();
  return typeof libraryId === 'number' ? `/libraries/${libraryId}` : '/';
}

function browseFilterForRoute(): BrowseFilter | undefined {
  if (state.route.page !== 'browse-detail') {
    return undefined;
  }
  const route = state.route;

  if (route.kind === 'collection') {
    const collection = collectionSummaries().find((entry) => entry.id === route.key);
    if (!collection) {
      return undefined;
    }

    return {
      kind: 'collection',
      label: collection.name,
      itemIds: collection.item_ids,
      overview: collection.overview,
      artworkUrl: collection.backdrop_url ?? collection.artwork_url,
    };
  }

  if (route.kind === 'playlist') {
    return {
      kind: 'playlist',
      label: route.key,
      itemIds: [],
      overview: 'No playlist items are available yet.',
    };
  }

  const category = categorySummaries().find((entry) => entry.genre === route.key);
  if (!category) {
    return undefined;
  }

  return {
    kind: 'category',
    label: category.genre,
    itemIds: category.items.map((item) => item.id),
    overview: category.items.slice(0, 5).map((item) => item.display_title).join(' · '),
  };
}

function renderBrowseFilterDetail(): string {
  const filter = state.route.page === 'browse-detail' ? browseFilterForRoute() : state.browseFilter;
  if (!filter) {
    if (state.libraryItemsLoading) {
      return '<div class="empty-state">Loading library items…</div>';
    }
    return '<div class="empty-state">This page is no longer available for the current library.</div>';
  }

  const allowedIds = new Set(filter.itemIds);
  const items = topLevelLibraryItems().filter((item) => allowedIds.has(item.id));
  const artworkStyle = filter.artworkUrl
    ? `style="--home-feature-image: url('${escapeHtml(filter.artworkUrl)}');"`
    : '';
  const themeSongOption = currentThemeSongYouTubeTarget();

  return `
    <section class="browse-filter-detail">
      <div class="home-feature ${filter.artworkUrl ? 'has-artwork' : ''}" ${artworkStyle}>
        <div class="home-feature-copy">
          <p class="eyebrow">${escapeHtml(filter.kind === 'collection' ? 'Collection' : filter.kind === 'playlist' ? 'Playlist' : 'Category')}</p>
          <h2>${escapeHtml(filter.label)}</h2>
          <p>${escapeHtml(filter.overview ?? `${items.length} title${items.length === 1 ? '' : 's'} in this ${filter.kind}.`)}</p>
          <div class="hero-meta-row">
            <span class="tag">${items.length} title${items.length === 1 ? '' : 's'}</span>
          </div>
        </div>
        <div class="home-feature-actions">
          ${themeSongOption ? `<button type="button" class="secondary-button" id="play-youtube-theme-song">${renderButtonContent('Play Theme', 'volume-2')}</button>` : ''}
          <button type="button" class="secondary-button" id="clear-browse-filter">
            ${renderButtonContent('Back', 'arrow-left')}
          </button>
        </div>
      </div>
      <div class="item-grid">${items.map(renderItemCard).join('')}</div>
    </section>
  `;
}

function renderCollectionDetailPage(): string {
  const collection = collectionForRoute();
  if (!collection) {
    if (state.libraryItemsLoading) {
      return '<div class="empty-state">Loading collection…</div>';
    }
    return '<div class="empty-state">This collection is no longer available for the current library.</div>';
  }

  const items = itemsForCollection(collection);
  const themeSongOption = currentThemeSongYouTubeTarget();
  const posterUrl = collection.artwork_url ? resolveApiUrl(collection.artwork_url) : undefined;
  const overview = collection.overview ?? 'No description is stored for this collection yet.';

  return `
    <section class="item-page collection-page">
      <section class="item-hero collection-hero">
        <div class="detail-art item-poster collection-poster ${posterUrl ? 'has-image' : ''}">
          ${posterUrl ? `<img src="${escapeHtml(posterUrl)}" alt="${escapeHtml(collection.name)} poster" />` : renderIcon('image', 'audio-player-art-icon')}
        </div>
        <div class="detail-summary item-summary">
          <p class="eyebrow">Collection</p>
          <h2 class="item-title-fallback">${escapeHtml(collection.name)}</h2>
          <div class="hero-meta-row">
            <span class="tag">${items.length} title${items.length === 1 ? '' : 's'}</span>
          </div>
          ${renderCollapsibleText(overview, `collection-overview:${collection.id}`)}
          <div class="detail-actions">
            ${themeSongOption ? `<button type="button" class="secondary-button" id="play-youtube-theme-song">${renderButtonContent('Play Theme', 'volume-2')}</button>` : ''}
            <button type="button" class="secondary-button" id="clear-browse-filter">${renderButtonContent('Back', 'arrow-left')}</button>
          </div>
        </div>
      </section>

      <section class="panel page-panel item-section">
        <div class="section-heading section-heading-actions">
          <h3>Items</h3>
          <span class="muted">${items.length} item${items.length === 1 ? '' : 's'}</span>
        </div>
        ${items.length
          ? `<div class="item-grid hierarchy-item-grid">${items.map(renderItemCard).join('')}</div>`
          : '<div class="empty-state tight">No titles are currently linked to this collection.</div>'}
      </section>
    </section>
  `;
}

function renderCategoryDetailPage(): string {
  const category = categoryForRoute();
  if (!category) {
    if (state.libraryItemsLoading) {
      return '<div class="empty-state">Loading genre…</div>';
    }
    return '<div class="empty-state">This genre is no longer available for the current library.</div>';
  }

  const overview = category.items.slice(0, 5).map((item) => item.display_title).join(' · ')
    || 'No titles are currently linked to this genre.';

  return `
    <section class="item-page grouped-page category-detail-page">
      <section class="item-hero collection-hero">
        <div class="detail-art item-poster collection-poster">
          ${renderIcon('layout-grid', 'audio-player-art-icon')}
        </div>
        <div class="detail-summary item-summary">
          <p class="eyebrow">Genre</p>
          <h2 class="item-title-fallback">${escapeHtml(category.genre)}</h2>
          <div class="hero-meta-row">
            <span class="tag">${category.items.length} title${category.items.length === 1 ? '' : 's'}</span>
          </div>
          ${renderCollapsibleText(overview, `category-overview:${category.genre}`)}
          <div class="detail-actions">
            <button type="button" class="secondary-button" id="clear-browse-filter">${renderButtonContent('Back', 'arrow-left')}</button>
          </div>
        </div>
      </section>

      <section class="panel page-panel item-section">
        <div class="section-heading section-heading-actions">
          <h3>Items</h3>
          <span class="muted">${category.items.length} item${category.items.length === 1 ? '' : 's'}</span>
        </div>
        ${category.items.length
          ? `<div class="item-grid hierarchy-item-grid">${category.items.map(renderItemCard).join('')}</div>`
          : '<div class="empty-state tight">No titles are currently linked to this genre.</div>'}
      </section>
    </section>
  `;
}

function renderPlaylistDetailPage(): string {
  const route = state.route;
  const playlistName = route.page === 'browse-detail' && route.kind === 'playlist'
    ? route.key
    : 'Playlist';

  return `
    <section class="item-page grouped-page playlist-detail-page">
      <section class="item-hero collection-hero">
        <div class="detail-art item-poster collection-poster">
          ${renderIcon('play', 'audio-player-art-icon')}
        </div>
        <div class="detail-summary item-summary">
          <p class="eyebrow">Playlist</p>
          <h2 class="item-title-fallback">${escapeHtml(playlistName)}</h2>
          <div class="hero-meta-row">
            <span class="tag">0 titles</span>
          </div>
          <p>No playlist items are available yet.</p>
          <div class="detail-actions">
            <button type="button" class="secondary-button" id="clear-browse-filter">${renderButtonContent('Back', 'arrow-left')}</button>
          </div>
        </div>
      </section>

      <section class="panel page-panel item-section">
        <div class="section-heading section-heading-actions">
          <h3>Items</h3>
          <span class="muted">0 items</span>
        </div>
        <div class="empty-state tight">Playlist creation is planned. Items will appear here when playlists are available.</div>
      </section>
    </section>
  `;
}

function renderBrowseDetailPage(): string {
  if (state.route.page !== 'browse-detail') {
    return renderBrowseFilterDetail();
  }

  switch (state.route.kind) {
    case 'collection':
      return renderCollectionDetailPage();
    case 'category':
      return renderCategoryDetailPage();
    case 'playlist':
      return renderPlaylistDetailPage();
  }

  return renderBrowseFilterDetail();
}

function metadataBadgeMarkup(item: MediaItemSummary): string {
  if (item.metadata_refresh_state === 'pending') {
    return '<span class="media-card-status is-loading"><span class="loading-spinner" aria-hidden="true"></span></span>';
  }

  if (item.has_metadata) {
    return '';
  }

  return `<span class="media-card-status is-unmatched icon-only" title="Metadata is not linked yet" aria-label="Metadata is not linked yet">${renderIcon('triangle-alert', 'status-icon')}</span>`;
}

function itemCardSubtitle(item: MediaItemSummary): string | undefined {
  if (item.item_type === 'episode' && typeof item.episode_number === 'number') {
    return `Episode ${item.episode_number}`;
  }

  if (item.item_type === 'season' && typeof item.season_number === 'number') {
    return `Season ${item.season_number}`;
  }

  return undefined;
}

function renderItemCard(item: MediaItemSummary): string {
  const library = state.libraries.find((entry) => entry.id === item.library_id);
  const artworkUrl = getArtworkUrl(item.id, 'poster', item.artwork_updated_at);
  const cardSubtitle = itemCardSubtitle(item);
  const isSeasonEpisodeCard = state.route.page === 'item'
    && state.selectedItem?.item_type === 'season'
    && item.item_type === 'episode';
  const secondaryMeta = isSeasonEpisodeCard
    ? undefined
    : state.route.page === 'home' && typeof state.route.libraryId === 'number'
      ? humanizeItemType(item.item_type)
      : `${library?.name ?? 'Library'} · ${humanizeItemType(item.item_type)}`;
  const badgeMarkup = metadataBadgeMarkup(item);

  return `
    <button class="media-card ${item.item_type === 'episode' ? 'episode-card' : ''}" type="button" data-item-id="${item.id}" data-preview-item-id="${item.id}">
      <span class="media-card-art ${escapeHtml(item.media_kind)} ${escapeHtml(item.item_type)}" style="background-image: url('${escapeHtml(artworkUrl)}');">
        <span class="media-card-kind-row">
          ${badgeMarkup}
          <span class="media-card-kind">${renderIcon(selectedLibraryIcon(library?.kind), 'card-icon')}</span>
        </span>
        <span class="media-card-duration">${escapeHtml(formatChildCount(item))}</span>
      </span>
      <span class="media-card-title">${escapeHtml(item.display_title)}</span>
      ${cardSubtitle ? `<span class="media-card-subtitle">${escapeHtml(cardSubtitle)}</span>` : ''}
      ${secondaryMeta ? `<span class="media-card-meta">${escapeHtml(secondaryMeta)}</span>` : ''}
    </button>
  `;
}

type HomeFeaturePreview =
  | { kind: 'collection'; collection: MediaCollectionSummary }
  | { kind: 'item'; item: MediaItemSummary };

function homeFeaturePreview(): HomeFeaturePreview | undefined {
  if (state.route.page === 'browse-detail' && state.route.kind === 'collection') {
    const collection = collectionForRoute();
    return collection ? { kind: 'collection', collection } : undefined;
  }

  if (state.route.page === 'home' && state.homeTab === 'collections') {
    const collections = collectionSummaries();
    const collection = collections.find((entry) => entry.id === state.homePreviewCollectionId) ?? collections[0];
    return collection ? { kind: 'collection', collection } : undefined;
  }

  const item = homePreviewItem();
  return item ? { kind: 'item', item } : undefined;
}

function homePreviewItem(): MediaItemSummary | undefined {
  const items = homePreviewCandidates();
  if (!items.length) {
    return undefined;
  }

  return showPreviewItemForHighlight(items.find((item) => item.id === state.homePreviewItemId) ?? items[0]);
}

function homePreviewCandidates(): MediaItemSummary[] {
  if (state.route.page === 'browse-detail') {
    return browseItemsForRoute();
  }

  switch (state.homeTab) {
    case 'library':
      return filteredTopLevelLibraryItems();
    case 'collections': {
      return filteredTopLevelLibraryItems();
    }
    case 'categories': {
      const seen = new Set<number>();
      const categoryItems = categorySummaries().flatMap((category) => category.items).filter((item) => {
        if (seen.has(item.id)) {
          return false;
        }
        seen.add(item.id);
        return true;
      });
      return categoryItems.length ? categoryItems : filteredTopLevelLibraryItems();
    }
    default: {
      const shelfItems = (state.home?.shelves ?? []).flatMap((shelf) => shelf.items);
      return shelfItems.length ? shelfItems : filteredTopLevelLibraryItems();
    }
  }
}

function pageBackdropUrlForItem(item: Pick<MediaItemSummary, 'id' | 'backdrop_url' | 'artwork_updated_at'> | undefined): string | undefined {
  return item?.backdrop_url
    ? getArtworkUrl(item.id, 'backdrop', item.artwork_updated_at)
    : undefined;
}

function pageBackdropUrlForCollection(collection: Pick<MediaCollectionSummary, 'backdrop_url' | 'artwork_url'> | undefined): string | undefined {
  const artworkUrl = collection?.backdrop_url ?? collection?.artwork_url;
  return artworkUrl ? resolveApiUrl(artworkUrl) : undefined;
}

function pageBackdropUrlForHomePreview(preview: HomeFeaturePreview | undefined): string | undefined {
  if (!preview) {
    return undefined;
  }

  return preview.kind === 'collection'
    ? pageBackdropUrlForCollection(preview.collection)
    : pageBackdropUrlForItem(preview.item);
}

function renderHomeFeature(): string {
  const preview = homeFeaturePreview();
  if (!preview) {
    return '';
  }

  if (preview.kind === 'collection') {
    const collection = preview.collection;
    const backdropUrl = pageBackdropUrlForCollection(collection);
    return `
      <section class="home-feature${backdropUrl ? ' has-artwork' : ''}" ${backdropUrl ? `style="--home-feature-image: url('${escapeHtml(backdropUrl)}');"` : ''}>
        <div class="home-feature-copy">
          <p class="eyebrow">Collection</p>
          <h2>${escapeHtml(collection.name)}</h2>
          <p>${escapeHtml(collection.overview ?? `${collection.item_count} title${collection.item_count === 1 ? '' : 's'} in this collection.`)}</p>
          <div class="hero-meta-row">
            <span class="tag">${collection.item_count} title${collection.item_count === 1 ? '' : 's'}</span>
          </div>
        </div>
        <button type="button" class="secondary-button home-feature-action" data-collection-filter="${escapeHtml(collection.id)}">
          ${renderButtonContent('Open', 'arrow-right')}
        </button>
      </section>
    `;
  }

  const item = preview.item;
  const backdropUrl = pageBackdropUrlForItem(item);
  const logoUrl = item.logo_url ? getArtworkUrl(item.id, 'logo', item.artwork_updated_at) : undefined;
  const library = state.libraries.find((entry) => entry.id === item.library_id);
  const genreMarkup = item.genres.slice(0, 3).map((genre) => `<span class="tag">${escapeHtml(genre)}</span>`).join('');

  return `
    <section class="home-feature${backdropUrl ? ' has-artwork' : ''}" ${backdropUrl ? `style="--home-feature-image: url('${escapeHtml(backdropUrl)}');"` : ''}>
      <div class="home-feature-copy">
        ${logoUrl
          ? `<img class="home-feature-logo" src="${escapeHtml(logoUrl)}" alt="${escapeHtml(item.display_title)}" />`
          : `<h2>${escapeHtml(item.display_title)}</h2>`}
        <p>${escapeHtml(item.overview ?? `${humanizeItemType(item.item_type)} from ${library?.name ?? 'your library'}.`)}</p>
        <div class="hero-meta-row">
          ${genreMarkup}
          <span class="tag">${escapeHtml(formatChildCount(item))}</span>
        </div>
      </div>
      <button type="button" class="secondary-button home-feature-action" data-item-id="${item.id}">
        ${renderButtonContent('Open', 'arrow-right')}
      </button>
    </section>
  `;
}

function renderSearchResults(): string {
  if (!state.searchResults.length) {
    return '<section class="shelf"><div class="empty-state">No media items matched the current search.</div></section>';
  }

  return `
    <section class="search-results-section">
      <div class="shelf-header">
        <h3>Search results</h3>
        <span>${state.searchResults.length} matches</span>
      </div>
      <div class="search-results-list">
        ${state.searchResults.map((item) => {
          const posterUrl = getArtworkUrl(item.id, 'poster', item.artwork_updated_at);
          const library = state.libraries.find((entry) => entry.id === item.library_id);
          return `
            <button type="button" class="search-result-row" data-item-id="${item.id}" data-preview-item-id="${item.id}">
              <span class="search-result-thumb" style="background-image: url('${escapeHtml(posterUrl)}');"></span>
              <span class="search-result-copy">
                <strong>${escapeHtml(item.display_title)}</strong>
                <span>${escapeHtml(`${library?.name ?? 'Library'} · ${humanizeItemType(item.item_type)} · ${formatChildCount(item)}`)}</span>
                ${item.overview ? `<small>${escapeHtml(item.overview)}</small>` : ''}
              </span>
            </button>
          `;
        }).join('')}
      </div>
    </section>
  `;
}

function renderShelfStack(): string {
  const shelves = (state.home?.shelves ?? []).filter((shelf) => shelf.items.length);
  if (!shelves.length) {
    return '<section class="shelf"><div class="empty-state">No shelves are available yet. Add a library to get started.</div></section>';
  }

  return shelves
    .map((shelf) => `
      <section class="shelf">
        <div class="shelf-header">
          <h3>${escapeHtml(shelf.title)}</h3>
          <span>${shelf.items.length} items</span>
        </div>
        <div class="shelf-row-shell">
              <button type="button" class="shelf-scroll-button" data-shelf-scroll="${escapeHtml(shelf.id)}:-1" title="Scroll left">${renderIcon('chevron-left')}</button>
              <div class="shelf-row" data-shelf-row="${escapeHtml(shelf.id)}">${shelf.items.map(renderItemCard).join('')}</div>
              <button type="button" class="shelf-scroll-button" data-shelf-scroll="${escapeHtml(shelf.id)}:1" title="Scroll right">${renderIcon('chevron-right')}</button>
            </div>
      </section>
    `)
    .join('');
}

function renderHomeTabs(): string {
  const tabs: Array<{ id: HomeBrowseTab; label: string }> = [
    { id: 'recommended', label: 'Recommended' },
    { id: 'library', label: 'Library' },
    { id: 'collections', label: 'Collections' },
    { id: 'playlists', label: 'Playlists' },
    { id: 'categories', label: 'Categories' },
  ];

  return `
    <nav class="browse-tabs" aria-label="Browse views">
      ${tabs.map((tab) => `
        <button
          type="button"
          class="browse-tab-button ${state.homeTab === tab.id ? 'active' : ''}"
          data-home-tab="${tab.id}"
        >
          ${escapeHtml(tab.label)}
        </button>
      `).join('')}
    </nav>
  `;
}

export function renderLibraryOverview(): string {
  const library = activeLibrary();
  const refreshProgress = library ? libraryRefreshProgress(library) : undefined;
  const stalePending = library ? Math.max(0, library.metadata_refresh_pending - activeLibraryPendingRefreshCount(library.id)) : 0;

  if (!library) {
    return `
      <section class="panel page-panel library-overview-panel">
        <div class="library-overview-grid">
          <article class="library-stat-card">
            <span class="label">Libraries</span>
            <strong>${state.libraries.length}</strong>
          </article>
          <article class="library-stat-card">
            <span class="label">Items</span>
            <strong>${topLevelLibraryItems().length}</strong>
          </article>
          <article class="library-stat-card">
            <span class="label">Status</span>
            <strong>${state.libraries.some((entry) => entry.status === 'never_scanned') ? 'Pending scans' : 'Ready'}</strong>
          </article>
        </div>
      </section>
    `;
  }

  return `
    <section class="panel page-panel library-overview-panel">
      <div class="library-overview-header">
        <div>
          <p class="eyebrow">Library overview</p>
          <h3>${escapeHtml(library.name)}</h3>
        </div>
        <div class="library-overview-actions">
          ${refreshProgress
            ? `<span class="tag warning">Refreshing metadata ${refreshProgress.completed}/${refreshProgress.total}</span>`
            : ''}
          <div class="library-status-tags">
          <span class="tag ${library.status === 'available' ? 'success' : library.status === 'never_scanned' ? 'warning' : ''}">${escapeHtml(libraryStatusLabel(library.status))}</span>
          <span class="tag">${library.total_files} file${library.total_files === 1 ? '' : 's'}</span>
          </div>
        </div>
      </div>
      <div class="library-overview-grid">
        <article class="library-stat-card">
          <span class="label">Top-level items</span>
          <strong>${topLevelLibraryItems().length}</strong>
        </article>
        <article class="library-stat-card">
          <span class="label">Video files</span>
          <strong>${library.video_files}</strong>
        </article>
        <article class="library-stat-card">
          <span class="label">Folders</span>
          <strong>${library.paths.length}</strong>
        </article>
        <article class="library-stat-card">
          <span class="label">Last scanned</span>
          <strong>${escapeHtml(formatTimestamp(library.last_scanned_at))}</strong>
        </article>
      </div>
      ${library.error ? `<p class="muted library-overview-note">${escapeHtml(library.error)}</p>` : ''}
      ${library.status === 'never_scanned' ? '<p class="muted library-overview-note">This library has not been scanned yet. It will populate after the next catalog scan starts.</p>' : ''}
      ${refreshProgress
        ? `<p class="muted library-overview-note">Metadata refresh progress: ${refreshProgress.completed}/${refreshProgress.total}${refreshProgress.failed ? ` (${refreshProgress.failed} failed)` : ''}. Artwork and item cards update automatically as each item completes.</p>`
        : ''}
      ${stalePending > 0
        ? `<p class="muted library-overview-note">${stalePending} item${stalePending === 1 ? ' is' : 's are'} still marked pending without an active refresh worker. Open Settings → Metadata dashboard to inspect the affected items and errors.</p>`
        : ''}
    </section>
  `;
}

function renderLibraryTab(): string {
  const items = filteredTopLevelLibraryItems();
  const library = activeLibrary();
  const isSpecificLibrary = state.route.page === 'home' && typeof state.route.libraryId === 'number';

  if (!items.length) {
    if (state.libraryItemsLoading) {
      return '<div class="empty-state">Loading library items…</div>';
    }

    if (state.browseFilter) {
      return `<div class="empty-state">No items matched the current ${escapeHtml(state.browseFilter.kind)} filter.</div>`;
    }

    if (library?.status === 'never_scanned') {
      return '<div class="empty-state">This library has not been scanned yet. The show, season, and episode hierarchy will appear after the first scan completes.</div>';
    }

    if (library?.status && library.status !== 'available') {
      return `<div class="empty-state">This library is not ready yet: ${escapeHtml(libraryStatusLabel(library.status))}.</div>`;
    }

    return '<div class="empty-state">No browseable items are available yet for this library.</div>';
  }

  return `
    <section class="browse-section">
      <div class="shelf-header browse-section-header">
        <h3>${isSpecificLibrary ? 'All items' : 'All libraries'}</h3>
        <span>${items.length} top-level item${items.length === 1 ? '' : 's'}</span>
      </div>
      ${state.browseFilter ? `
        <div class="active-filter-bar">
          <span class="tag success">${escapeHtml(state.browseFilter.kind === 'category' ? 'Category' : 'Collection')}</span>
          <strong>${escapeHtml(state.browseFilter.label)}</strong>
          <button type="button" class="secondary-button" id="clear-browse-filter">${renderButtonContent('Clear filter', 'x')}</button>
        </div>
      ` : ''}
      <div class="item-grid">${items.map(renderItemCard).join('')}</div>
    </section>
  `;
}

function renderCollectionsTab(): string {
  const collections = collectionSummaries();
  if (!collections.length) {
    return '<div class="empty-state">No linked collection data is available yet for this library.</div>';
  }

  return `
    <section class="item-grid">
      ${collections.map((collection) => {
        const posterUrl = collection.artwork_url ?? collection.backdrop_url;
        return `
          <button
            type="button"
            class="media-card collection-browse-card"
            data-collection-filter="${escapeHtml(collection.id)}"
            data-preview-collection-id="${escapeHtml(collection.id)}"
          >
            <span class="media-card-art collection" style="${posterUrl ? `background-image: url('${escapeHtml(resolveApiUrl(posterUrl))}');` : ''}">
              <span class="media-card-kind-row">
                <span class="media-card-kind">${renderIcon('image', 'card-icon')}</span>
              </span>
              <span class="media-card-duration">${collection.item_count} title${collection.item_count === 1 ? '' : 's'}</span>
            </span>
            <span class="media-card-title">${escapeHtml(collection.name)}</span>
          </button>
        `;
      }).join('')}
    </section>
  `;
}

function renderPlaylistsTab(): string {
  return `
    <section class="category-grid">
      <button
        type="button"
        class="category-card panel filter-card-button"
        data-playlist-filter="Playlists"
      >
        <div class="category-card-header">
          <strong>Playlists</strong>
          <span class="tag">0 titles</span>
        </div>
        <p class="muted">Playlist creation is planned. Items will appear here when playlists are available.</p>
      </button>
    </section>
  `;
}

function renderCategoriesTab(): string {
  const categories = categorySummaries();
  if (!categories.length) {
    return '<div class="empty-state">No genre metadata is available yet for the current library.</div>';
  }

  return `
    <section class="category-grid">
      ${categories.map((category) => `
        <button
          type="button"
          class="category-card panel filter-card-button"
          data-category-filter="${escapeHtml(category.genre)}"
          ${category.items[0] ? `data-preview-item-id="${category.items[0].id}"` : ''}
        >
          <div class="category-card-header">
            <strong>${escapeHtml(category.genre)}</strong>
            <span class="tag">${category.count} title${category.count === 1 ? '' : 's'}</span>
          </div>
          <p class="muted">${escapeHtml(category.items.slice(0, 3).map((item) => item.display_title).join(' · ') || 'No titles yet')}</p>
        </button>
      `).join('')}
    </section>
  `;
}

function renderHomeTabContent(): string {
  if (state.route.page === 'browse-detail') {
    return renderBrowseDetailPage();
  }

  if (state.browseFilter) {
    return renderBrowseFilterDetail();
  }

  if (state.showFullSearchResults && state.searchQuery.trim()) {
    return renderSearchResults();
  }

  switch (state.homeTab) {
    case 'library':
      return renderLibraryTab();
    case 'collections':
      return renderCollectionsTab();
    case 'playlists':
      return renderPlaylistsTab();
    case 'categories':
      return renderCategoriesTab();
    default:
      return renderShelfStack();
  }
}

function renderPageNavbar(eyebrow: string, title: string, description: string, actions = ''): string {
  return `
    <header class="content-navbar panel page-panel">
      <div class="content-navbar-copy">
        <p class="eyebrow">${escapeHtml(eyebrow)}</p>
        <h2>${escapeHtml(title)}</h2>
        <p class="muted">${escapeHtml(description)}</p>
      </div>
      ${actions ? `<div class="content-navbar-actions">${actions}</div>` : ''}
    </header>
  `;
}

function renderHomePage(): string {
  if (state.route.page === 'browse-detail') {
    return `
      ${renderHomeNavbar()}
      ${renderBrowseDetailPage()}
    `;
  }

  return `
    ${renderHomeNavbar()}
    ${renderHomeFeature()}
    <section class="shelf-stack panel page-panel">${renderHomeTabContent()}</section>
  `;
}

function renderMetadataSearchResults(): string {
  const selectedItem = state.selectedItem;
  if (!selectedItem) {
    return '';
  }

  if (!state.metadataSearchResults.length) {
    return '<div class="empty-state tight">Search metadata providers to link rich metadata and artwork.</div>';
  }

  return state.metadataSearchResults
    .map((result) => `
      <article class="metadata-search-card">
        ${result.artwork_url ? `<img class="metadata-search-poster" src="${escapeHtml(resolveApiUrl(result.artwork_url))}" alt="" loading="lazy" />` : ''}
        <div>
          <strong>${escapeHtml(result.title)}</strong>
          <p>${escapeHtml(result.overview ?? 'No overview available.')}</p>
          <div class="metadata-match-meta">
            <span>${escapeHtml(providerDisplayName(result.provider_id))}</span>
            ${providerAttributionLogo(result.provider_id) ? `<img class="metadata-attribution-logo" src="${escapeHtml(providerAttributionLogo(result.provider_id) ?? '')}" alt="" loading="lazy" />` : ''}
            <span>${result.release_year ?? 'Unknown year'}</span>
            <span>${escapeHtml(result.media_type)}</span>
            ${typeof result.score === 'number' ? `<span>${Math.round(result.score * 100)}% match</span>` : ''}
          </div>
        </div>
        <button
          type="button"
          class="secondary-button"
          data-link-metadata="${selectedItem.id}:${escapeHtml(result.provider_id)}:${escapeHtml(result.external_id)}:${escapeHtml(result.media_type)}"
        >
          ${renderButtonContent('Link', 'link-2')}
        </button>
      </article>
    `)
    .join('');
}

function parseMetadataLanguageInput(value: FormDataEntryValue | null): string[] {
  const languages = String(value ?? '')
    .split(',')
    .map((language) => language.trim())
    .filter(Boolean);
  return languages.length ? languages : ['en-US'];
}

function selectedItemMetadataProviderOptions(): MetadataProviderStatus[] {
  const itemType = state.selectedItem?.item_type;
  const libraryKind = itemType === 'show' ? 'shows' : itemType === 'movie' ? 'movies' : undefined;
  return (state.selectedItemMetadata?.providers ?? state.metadataProviders)
    .filter((provider) => provider.role !== 'secondary')
    .filter((provider) => provider.configured && provider.implemented)
    .filter((provider) => !libraryKind || provider.supported_kinds.includes(libraryKind));
}

function defaultMetadataSearchProviderIds(): string[] {
  const providers = selectedItemMetadataProviderOptions();
  const librarySettings = activeLibrarySettings();
  const libraryProviderIds = librarySettings?.metadata_providers ?? [];
  const selectedLibraryProviders = providers
    .filter((provider) => libraryProviderIds.includes(provider.id))
    .map((provider) => provider.id);
  return librarySettings ? selectedLibraryProviders : providers.map((provider) => provider.id);
}

function selectedItemDefaultMetadataTitle(): string {
  return state.selectedItem?.display_title.trim()
    || state.selectedItemMetadata?.matches[0]?.title?.trim()
    || '';
}

function selectedItemDefaultMetadataYear(): string {
  const year = state.selectedItem?.release_year ?? state.selectedItemMetadata?.matches[0]?.release_year;
  return typeof year === 'number' ? String(year) : '';
}

function defaultMetadataSearchLanguage(): string {
  const librarySettings = activeLibrarySettings();
  if (librarySettings?.metadata_language_mode === 'manual') {
    return normalizedMetadataLanguages(librarySettings.metadata_languages)[0] ?? 'en-US';
  }
  return state.bootstrap?.current_user?.preferred_metadata_languages?.[0]
    ?? state.metadataProviders.find((provider) => provider.configured)?.language
    ?? 'en-US';
}

function providerAttributionLogo(providerId: string): string | undefined {
  const provider = (state.selectedItemMetadata?.providers ?? state.metadataProviders)
    .find((entry) => entry.id === providerId);
  return provider?.logo_dark_url ?? provider?.logo_light_url;
}

function renderMetadataSearchProviderControls(): string {
  const providers = selectedItemMetadataProviderOptions();
  if (!providers.length) {
    return '';
  }

  const selectedProviders = state.metadataSearchProviders.length
    ? state.metadataSearchProviders
    : defaultMetadataSearchProviderIds();

  return `
    <div class="metadata-provider-picker">
      ${providers.map((provider) => `
        <label class="checkbox-inline">
          <input
            name="metadataSearchProvider"
            type="checkbox"
            value="${escapeHtml(provider.id)}"
            ${selectedProviders.includes(provider.id) ? 'checked' : ''}
          />
          <span>${escapeHtml(provider.display_name)}</span>
        </label>
      `).join('')}
    </div>
  `;
}

function renderSearchPopover(): string {
  if (!state.searchQuery.trim() || state.showFullSearchResults) {
    return '';
  }

  if (!state.searchResults.length) {
    return '<div class="search-popover panel"><div class="empty-state tight">No media items matched the current search.</div></div>';
  }

  return `
    <div class="search-popover panel">
      <div class="search-popover-header">
        <strong>Search results</strong>
        <span>${state.searchResults.length} match${state.searchResults.length === 1 ? '' : 'es'}</span>
      </div>
      <div class="search-results-list compact">
        ${state.searchResults.slice(0, 8).map((item) => {
          const posterUrl = getArtworkUrl(item.id, 'poster', item.artwork_updated_at);
          const library = state.libraries.find((entry) => entry.id === item.library_id);
          return `
            <button type="button" class="search-result-row" data-item-id="${item.id}" data-preview-item-id="${item.id}">
              <span class="search-result-thumb" style="background-image: url('${escapeHtml(posterUrl)}');"></span>
              <span class="search-result-copy">
                <strong>${escapeHtml(item.display_title)}</strong>
                <span>${escapeHtml(`${library?.name ?? 'Library'} · ${humanizeItemType(item.item_type)}`)}</span>
              </span>
            </button>
          `;
        }).join('')}
      </div>
    </div>
  `;
}

function renderHomeNavbar(): string {
  const library = activeLibrary();
  const libraryRefreshPending = library ? Boolean(libraryRefreshProgress(library)) : false;

  return `
    <header class="home-navbar">
      ${renderHomeTabs()}
      <div class="home-navbar-tools">
        <form id="search-form" class="search-form">
          <input id="search-input" name="search" type="search" value="${escapeHtml(state.searchQuery)}" placeholder="Search" autocomplete="off" />
          <button id="search-toggle" type="submit" class="icon-button search-toggle-button" title="Search" aria-label="Search">${renderIcon('search')}</button>
        </form>
        ${library
          ? `
            <button type="button" class="icon-button secondary-button" id="scan-active-library" title="Scan library" aria-label="Scan library">${renderIcon('folder-sync')}</button>
            <button type="button" class="icon-button secondary-button" id="refresh-active-library-metadata" title="Refresh metadata" aria-label="Refresh metadata" ${libraryRefreshPending ? 'disabled' : ''}>${renderIcon('database-zap')}</button>
          `
          : ''}
      </div>
      ${renderSearchPopover()}
    </header>
  `;
}

function renderLinkedMetadataSummary(): string {
  const matches = state.selectedItemMetadata?.matches ?? [];
  const linkedMatch = matches.find((match) => match.relation_kind === 'primary') ?? matches[0];
  if (!linkedMatch) {
    return '<div class="empty-state tight">No external metadata is linked yet.</div>';
  }

  const metadataRefreshPending = itemIsMetadataPending(state.selectedItem);
  const refreshStateLabel = metadataRefreshPending || linkedMatch.refresh_state === 'pending'
    ? 'Refreshing'
    : linkedMatch.refresh_state === 'error'
      ? 'Refresh failed'
      : 'Up to date';
  const providersById = new Map(
    (state.selectedItemMetadata?.providers ?? state.metadataProviders).map((provider) => [provider.id, provider]),
  );
  const contributingProviderIds = [
    linkedMatch.provider_id,
    ...matches.map((match) => match.provider_id).filter((providerId) => providerId !== linkedMatch.provider_id),
  ].filter((providerId, index, providerIds) => providerIds.indexOf(providerId) === index);
  const providerTags = contributingProviderIds
    .map((providerId) => {
      const className = providerId === linkedMatch.provider_id ? 'tag success' : 'tag';
      return `<span class="${className}">${escapeHtml(providerId)}</span>`;
    })
    .join('');
  const attributions = contributingProviderIds
    .map((providerId) => providersById.get(providerId))
    .filter((provider): provider is MetadataProviderStatus => Boolean(provider?.attribution_text))
    .map((provider) => {
      const logoUrl = providerAttributionLogo(provider.id);
      return `<a class="metadata-attribution" href="${escapeHtml(provider.attribution_url)}" target="_blank" rel="noreferrer">${logoUrl ? `<img src="${escapeHtml(logoUrl)}" alt="" loading="lazy" />` : ''}${escapeHtml(provider.attribution_text)}</a>`;
    })
    .join('');

  return `
    <div class="metadata-current-link">
      ${providerTags}
      <span class="tag">${escapeHtml(linkedMatch.media_type ?? 'linked')}</span>
      <span class="tag ${metadataRefreshPending || linkedMatch.refresh_state === 'pending' ? 'warning' : linkedMatch.refresh_state === 'error' ? 'danger-tag' : ''}">${escapeHtml(refreshStateLabel)}</span>
      ${linkedMatch.release_year ? `<span class="tag">${linkedMatch.release_year}</span>` : ''}
      ${linkedMatch.locale_key ? `<span class="tag">${escapeHtml(linkedMatch.locale_key)}</span>` : ''}
      <span class="metadata-current-copy">
        <strong>${escapeHtml(linkedMatch.title ?? linkedMatch.external_id)}</strong>
        <span class="muted">Last refreshed ${escapeHtml(formatTimestamp(linkedMatch.last_refreshed_at ?? linkedMatch.updated_at))}</span>
        ${attributions}
        ${linkedMatch.refresh_error ? `<span class="metadata-refresh-error">${escapeHtml(linkedMatch.refresh_error)}</span>` : ''}
      </span>
    </div>
  `;
}

function subtitleLanguage(trackLabel: string): string {
  const normalized = trackLabel.trim().toLowerCase();
  if (/^[a-z]{2,3}$/.test(normalized)) {
    return normalized;
  }

  return 'en';
}

function renderMetadataDashboard(): string {
  const filteredItems = filteredMetadataDashboardItems();
  const summary = metadataDashboardSummary(state.dashboardItems);
  const itemTypes = [...new Set(state.dashboardItems.map((item) => item.item_type))].sort((left, right) => left.localeCompare(right));

  return `
    <section class="panel page-panel metadata-dashboard-panel">
      <div class="section-heading section-heading-actions">
        <div>
          <h3>Metadata dashboard</h3>
          <p class="muted">Browse every item, identify failed refreshes, and spot pending items that no longer have an active worker.</p>
        </div>
        <div class="provider-tags">
          <span class="tag">${state.dashboardItems.length} total</span>
          <span class="tag ${summary.failed ? 'danger-tag' : ''}">${summary.failed} failed</span>
          <span class="tag warning">${summary.pending} active</span>
          <span class="tag ${summary.stalled ? 'warning' : ''}">${summary.stalled} stalled</span>
          <span class="tag">${summary.unmatched} unmatched</span>
        </div>
      </div>
      <form id="metadata-dashboard-filter-form" class="settings-form metadata-dashboard-filter-form">
        <div class="form-row metadata-dashboard-filter-grid">
          <label>Library
            <select name="dashboard_library_id">
              <option value="" ${state.metadataDashboardFilters.libraryId ? '' : 'selected'}>All libraries</option>
              ${state.libraries.map((library) => `<option value="${library.id}" ${state.metadataDashboardFilters.libraryId === String(library.id) ? 'selected' : ''}>${escapeHtml(library.name)}</option>`).join('')}
            </select>
          </label>
          <label>Item type
            <select name="dashboard_item_type">
              <option value="" ${state.metadataDashboardFilters.itemType ? '' : 'selected'}>All item types</option>
              ${itemTypes.map((itemType) => `<option value="${escapeHtml(itemType)}" ${state.metadataDashboardFilters.itemType === itemType ? 'selected' : ''}>${escapeHtml(humanizeItemType(itemType))}</option>`).join('')}
            </select>
          </label>
          <label>Refresh state
            <select name="dashboard_refresh_state">
              <option value="" ${state.metadataDashboardFilters.refreshState ? '' : 'selected'}>All states</option>
              ${[
                ['error', 'Failed'],
                ['stalled', 'Pending without worker'],
                ['pending', 'Refreshing'],
                ['fresh', 'Up to date'],
                ['unmatched', 'Not linked'],
              ].map(([value, label]) => `<option value="${value}" ${state.metadataDashboardFilters.refreshState === value ? 'selected' : ''}>${label}</option>`).join('')}
            </select>
          </label>
        </div>
        <label>Search
          <input name="dashboard_search" value="${escapeHtml(state.metadataDashboardFilters.search)}" placeholder="Title, path, or refresh error" />
        </label>
        <div class="page-actions">
          <button type="submit">${renderButtonContent('Apply filters', 'search')}</button>
          <button type="button" class="secondary-button" id="clear-metadata-dashboard-filters">${renderButtonContent('Clear filters', 'x')}</button>
        </div>
      </form>
      ${filteredItems.length
        ? `<div class="table-shell metadata-dashboard-table-shell">
            <table class="data-table metadata-dashboard-table">
              <thead>
                <tr>
                  <th>Title</th>
                  <th>Type</th>
                  <th>Library</th>
                  <th>Refresh state</th>
                  <th>Artwork updated</th>
                  <th>Children</th>
                  <th></th>
                </tr>
              </thead>
              <tbody>${filteredItems.map((item) => {
            const library = state.libraries.find((entry) => entry.id === item.library_id);
            const refreshState = metadataDashboardRefreshState(item);
            return `
              <tr>
                <td>
                  <div class="table-title-cell">
                    <strong>${escapeHtml(item.display_title)}</strong>
                    <p class="muted metadata-dashboard-path">${escapeHtml(item.relative_path)}</p>
                    ${item.metadata_refresh_error ? `<p class="metadata-dashboard-error">${escapeHtml(item.metadata_refresh_error)}</p>` : ''}
                  </div>
                </td>
                <td>${escapeHtml(humanizeItemType(item.item_type))}</td>
                <td>${escapeHtml(library?.name ?? `Library ${item.library_id}`)}</td>
                <td><span class="tag ${refreshState === 'error' ? 'danger-tag' : refreshState === 'pending' || refreshState === 'stalled' ? 'warning' : refreshState === 'fresh' ? 'success' : ''}">${escapeHtml(metadataDashboardRefreshLabel(item))}</span></td>
                <td>${escapeHtml(formatTimestamp(item.artwork_updated_at))}</td>
                <td>${escapeHtml(formatChildCount(item))}</td>
                <td><button type="button" class="secondary-button" data-item-id="${item.id}">${renderButtonContent('Open item', 'arrow-left', 'end')}</button></td>
              </tr>
            `;
          }).join('')}</tbody>
            </table>
          </div>`
        : '<div class="empty-state tight">No items matched the current dashboard filters.</div>'}
    </section>
  `;
}

function renderSystemActivitiesPanel(): string {
  const activities = state.systemActivities.filter((activity) => activity.state !== 'completed' && activity.state !== 'failed');
  return `
    <section class="panel page-panel settings-activity-panel">
      <div class="section-heading section-heading-actions">
        <div>
          <h3>Backend activities</h3>
          <p class="muted">Active background work that the browser is polling.</p>
        </div>
        <span class="tag">${activities.length} active</span>
      </div>
      ${activities.length
        ? `<div class="settings-system-activity-list">${activities.map((activity) => {
            const progress = activityProgress(activity);
            return `
              <article class="settings-system-activity">
                <div class="settings-system-activity-header">
                  <div>
                    <strong>${escapeHtml(activity.label)}</strong>
                    <p class="muted">${escapeHtml(activity.scope)} · ${escapeHtml(activity.source)}</p>
                  </div>
                  <div class="provider-tags">
                    <span class="tag ${activity.state === 'running' ? 'warning' : ''}">${escapeHtml(activity.state)}</span>
                    ${activity.provider_id ? `<span class="tag">${escapeHtml(activity.provider_id)}</span>` : ''}
                  </div>
                </div>
                <div class="activity-progress-row">
                  <div class="activity-progress-bar" aria-hidden="true">
                    <span class="activity-progress-fill" style="--activity-progress: ${progress.percent}%;"></span>
                  </div>
                  <span class="muted">${progress.completed}/${progress.total}${progress.failed ? ` · ${progress.failed} failed` : ''}</span>
                </div>
              </article>
            `;
          }).join('')}</div>`
        : '<div class="empty-state tight">No background activities are running right now.</div>'}
    </section>
  `;
}

function renderLogViewer(): string {
  const logEntries = state.logsResponse?.entries ?? [];

  return `
    <section class="panel page-panel settings-log-panel">
      <div class="section-heading section-heading-actions">
        <div>
          <h3>Logs</h3>
          <p class="muted">Structured logs from ${escapeHtml(state.logsResponse?.log_path ?? 'the current log file')}.</p>
        </div>
        <button type="button" class="secondary-button" id="refresh-log-viewer">${renderButtonContent('Refresh logs', 'refresh-cw')}</button>
      </div>
      <form id="log-filter-form" class="settings-form log-filter-form">
        <div class="form-row log-filter-row">
          <label>Level
            <select name="log_level">
              <option value="" ${state.logFilters.level ? '' : 'selected'}>All levels</option>
              ${['TRACE', 'DEBUG', 'INFO', 'WARN', 'ERROR'].map((level) => `<option value="${level}" ${state.logFilters.level === level ? 'selected' : ''}>${level}</option>`).join('')}
            </select>
          </label>
          <label>Module<input name="log_module" value="${escapeHtml(state.logFilters.module)}" placeholder="koko::web::routes::media" /></label>
        </div>
        <div class="form-row log-filter-row">
          <label>From
            <input name="log_since" type="datetime-local" value="${escapeHtml(state.logFilters.since)}" />
          </label>
          <label>Until
            <input name="log_until" type="datetime-local" value="${escapeHtml(state.logFilters.until)}" />
          </label>
        </div>
        <label>Search<input name="log_search" value="${escapeHtml(state.logFilters.search)}" placeholder="message text, source path, or module" /></label>
        <div class="page-actions">
          <button type="submit">${renderButtonContent('Apply filters', 'search')}</button>
          <button type="button" class="secondary-button" id="clear-log-filters">${renderButtonContent('Clear filters', 'x')}</button>
        </div>
      </form>
      ${logEntries.length
        ? `<div class="table-shell">
            <table class="data-table log-entries-table">
              <thead>
                <tr>
                  <th>Time</th>
                  <th>Level</th>
                  <th>Module</th>
                  <th>Source</th>
                  <th>Message</th>
                </tr>
              </thead>
              <tbody>${logEntries.map((entry) => `
                <tr>
                  <td>${escapeHtml(entry.timestamp)}</td>
                  <td><span class="tag ${entry.level === 'ERROR' ? 'danger-tag' : entry.level === 'WARN' ? 'warning' : ''}">${escapeHtml(entry.level)}</span></td>
                  <td>${escapeHtml(entry.module)}</td>
                  <td class="muted">${escapeHtml(entry.source_file_path)}${typeof entry.line_number === 'number' ? `:${entry.line_number}` : ''}</td>
                  <td><pre class="log-entry-message">${escapeHtml(entry.message)}</pre></td>
                </tr>
              `).join('')}</tbody>
            </table>
          </div>`
        : '<div class="empty-state tight">No log entries matched the current filters.</div>'}
    </section>
  `;
}

function activeSettingsSection(): SettingsSection {
  return state.route.page === 'settings' ? state.route.section ?? 'general' : 'general';
}

function renderSettingsSectionNav(): string {
  const activeSection = activeSettingsSection();
  const sections: Array<{ id: SettingsSection; label: string; path: string }> = [
    { id: 'general', label: 'General', path: '/settings' },
    { id: 'libraries', label: 'Libraries', path: '/settings/libraries' },
    { id: 'providers', label: 'Providers', path: '/settings/providers' },
    { id: 'dashboard', label: 'Dashboard', path: '/settings/dashboard' },
    { id: 'logs', label: 'Logs', path: '/settings/logs' },
  ];

  return `
    <nav class="settings-section-nav panel page-panel" aria-label="Settings sections">
      ${sections.map((section) => `
        <button type="button" class="secondary-button ${activeSection === section.id ? 'active' : ''}" data-settings-section-path="${section.path}">
          ${escapeHtml(section.label)}
        </button>
      `).join('')}
    </nav>
  `;
}

function selectedItemPeople(): ItemMetadataPerson[] {
  return state.selectedItemMetadata?.matches[0]?.people ?? [];
}

function formatPersonDate(value?: string): string {
  if (!value) {
    return '';
  }

  const date = new Date(`${value}T00:00:00`);
  if (Number.isNaN(date.getTime())) {
    return value;
  }

  return date.toLocaleDateString('en-US', { month: 'short', day: 'numeric', year: 'numeric' });
}

function personAgeLabel(birthday?: string, deathday?: string): string | undefined {
  if (!birthday) {
    return undefined;
  }
  const birthDate = new Date(`${birthday}T00:00:00`);
  const endDate = deathday ? new Date(`${deathday}T00:00:00`) : new Date();
  if (Number.isNaN(birthDate.getTime()) || Number.isNaN(endDate.getTime())) {
    return undefined;
  }
  let age = endDate.getFullYear() - birthDate.getFullYear();
  const birthdayThisYear = new Date(endDate.getFullYear(), birthDate.getMonth(), birthDate.getDate());
  if (endDate < birthdayThisYear) {
    age -= 1;
  }
  return deathday ? `${age} at death` : `${age} years old`;
}

function renderPersonCredit(person: ItemMetadataPerson): string {
  const imageUrl = person.cached_image_path
    ? getPersonImageUrl(person.person_id)
    : person.image_url ? resolveApiUrl(person.image_url) : undefined;
  const subtitle = person.character_name || person.role || person.department || '';
  return `
    <button class="person-card" type="button" data-person-id="${person.person_id}">
      <span class="person-card-art ${imageUrl ? 'has-image' : ''}" ${imageUrl ? `style="background-image: url('${escapeHtml(imageUrl)}');"` : ''}>
        ${imageUrl ? '' : `<span>${escapeHtml(person.name.slice(0, 1).toUpperCase())}</span>`}
      </span>
      <span class="person-card-title">${escapeHtml(person.name)}</span>
      ${subtitle ? `<span class="person-card-subtitle">${escapeHtml(subtitle)}</span>` : ''}
    </button>
  `;
}

function renderPeopleRail(): string {
  const people = selectedItemPeople();
  if (!people.length) {
    return '';
  }

  return `
    <section class="panel page-panel item-section item-people-section">
      <div class="section-heading section-heading-actions">
        <h3>People</h3>
        <span class="muted">${people.length} credit${people.length === 1 ? '' : 's'}</span>
      </div>
      <div class="shelf-row-shell people-row-shell">
        <button type="button" class="shelf-scroll-button" data-shelf-scroll="people:-1" title="Scroll left">${renderIcon('chevron-left')}</button>
        <div class="people-row" data-shelf-row="people">
          ${people.map(renderPersonCredit).join('')}
        </div>
        <button type="button" class="shelf-scroll-button" data-shelf-scroll="people:1" title="Scroll right">${renderIcon('chevron-right')}</button>
      </div>
    </section>
  `;
}

function renderSelectedItemCollectionRails(): string {
  const rails = selectedItemCollectionRails();
  if (!rails.length) {
    return '';
  }

  return rails
    .map((rail, index) => {
      const rowId = `item-collection-${index}`;
      return `
        <section class="panel page-panel item-section item-collection-section">
          <div class="section-heading section-heading-actions">
            <div>
              <h3>${escapeHtml(rail.collection.name)}</h3>
              <p class="muted">Also in this collection</p>
            </div>
            <button type="button" class="secondary-button" data-collection-filter="${escapeHtml(rail.collection.id)}">${renderButtonContent('View collection', 'arrow-right')}</button>
          </div>
          <div class="shelf-row-shell">
            <button type="button" class="shelf-scroll-button" data-shelf-scroll="${rowId}:-1" title="Scroll left">${renderIcon('chevron-left')}</button>
            <div class="shelf-row" data-shelf-row="${rowId}">${rail.items.map(renderItemCard).join('')}</div>
            <button type="button" class="shelf-scroll-button" data-shelf-scroll="${rowId}:1" title="Scroll right">${renderIcon('chevron-right')}</button>
          </div>
        </section>
      `;
    })
    .join('');
}

interface PersonSeasonCreditGroup {
  season: MediaItemSummary;
  episodes: MediaItemSummary[];
}

interface PersonCreditGroup {
  root: MediaItemSummary;
  seasons: PersonSeasonCreditGroup[];
}

function itemSortKey(item: MediaItemSummary): string {
  const season = typeof item.season_number === 'number' ? String(item.season_number).padStart(5, '0') : '99999';
  const episode = typeof item.episode_number === 'number' ? String(item.episode_number).padStart(5, '0') : '99999';
  return `${season}:${episode}:${item.display_title.toLocaleLowerCase()}`;
}

function compareMediaItems(left: MediaItemSummary, right: MediaItemSummary): number {
  return itemSortKey(left).localeCompare(itemSortKey(right));
}

function personCreditRootItem(entry: MetadataPersonItemCredit): MediaItemSummary {
  return entry.hierarchy.find((item) => item.item_type === 'show')
    ?? entry.hierarchy[0]
    ?? entry.item;
}

function personCreditSeasonItem(entry: MetadataPersonItemCredit): MediaItemSummary | undefined {
  if (entry.item.item_type === 'season') {
    return entry.item;
  }

  if (entry.item.item_type !== 'episode') {
    return undefined;
  }

  return [...entry.hierarchy].reverse().find((item) => item.item_type === 'season');
}

function personCreditGroups(credits: MetadataPersonItemCredit[]): PersonCreditGroup[] {
  const groupsByRootId = new Map<number, PersonCreditGroup>();

  credits.forEach((entry) => {
    const root = personCreditRootItem(entry);
    if (!groupsByRootId.has(root.id)) {
      groupsByRootId.set(root.id, { root, seasons: [] });
    }

    const group = groupsByRootId.get(root.id)!;
    const season = personCreditSeasonItem(entry);
    if (!season) {
      return;
    }

    let seasonGroup = group.seasons.find((candidate) => candidate.season.id === season.id);
    if (!seasonGroup) {
      seasonGroup = { season, episodes: [] };
      group.seasons.push(seasonGroup);
    }

    if (entry.item.item_type === 'episode' && !seasonGroup.episodes.some((episode) => episode.id === entry.item.id)) {
      seasonGroup.episodes.push(entry.item);
    }
  });

  return [...groupsByRootId.values()]
    .map((group) => ({
      ...group,
      seasons: group.seasons
        .map((seasonGroup) => ({
          ...seasonGroup,
          episodes: seasonGroup.episodes.sort(compareMediaItems),
        }))
        .sort((left, right) => compareMediaItems(left.season, right.season)),
    }))
    .sort((left, right) => left.root.display_title.localeCompare(right.root.display_title));
}

function renderPersonCreditGroup(group: PersonCreditGroup): string {
  const seasonCount = group.seasons.length;
  const episodeCount = group.seasons.reduce((total, season) => total + season.episodes.length, 0);
  const traySummary = [
    seasonCount ? `${seasonCount} season${seasonCount === 1 ? '' : 's'}` : '',
    episodeCount ? `${episodeCount} episode${episodeCount === 1 ? '' : 's'}` : '',
  ].filter(Boolean).join(' · ');

  return `
    <article class="person-credit-card" data-person-credit-card data-person-credit-id="${group.root.id}">
      ${renderItemCard(group.root)}
    </article>
    ${group.seasons.length ? `
      <div class="person-credit-tray person-season-tray" data-person-credit-tray data-person-credit-id="${group.root.id}">
        <div class="person-credit-tray-heading">
          <span>${escapeHtml(traySummary || 'Credits')}</span>
          <button class="person-credit-tray-close" type="button" data-close-person-credit-tray title="Collapse row" aria-label="Collapse row">${renderIcon('x')}</button>
        </div>
        <div class="person-season-credit-grid">
          ${group.seasons.map((seasonGroup) => `
            <article class="person-season-credit-card" data-person-season-credit-card data-person-season-credit-id="${seasonGroup.season.id}">
              ${renderItemCard(seasonGroup.season)}
            </article>
            ${seasonGroup.episodes.length ? `
              <div class="person-credit-tray person-episode-tray" data-person-season-credit-tray data-person-season-credit-id="${seasonGroup.season.id}">
                <div class="person-credit-tray-heading">
                  <span>${seasonGroup.episodes.length} episode${seasonGroup.episodes.length === 1 ? '' : 's'}</span>
                  <button class="person-credit-tray-close" type="button" data-close-person-season-credit-tray title="Collapse row" aria-label="Collapse row">${renderIcon('x')}</button>
                </div>
                <div class="person-episode-credit-grid">
                  ${seasonGroup.episodes.map(renderItemCard).join('')}
                </div>
              </div>
            ` : ''}
          `).join('')}
        </div>
      </div>
    ` : ''}
  `;
}

function renderPersonPage(): string {
  const response = state.selectedPerson;
  if (!response) {
    return '<section class="panel page-panel"><div class="empty-state">Loading person details…</div></section>';
  }

  const personImageUrl = response.person.cached_image_path
    ? getPersonImageUrl(response.person.id)
    : response.person.image_url ? resolveApiUrl(response.person.image_url) : undefined;
  const credits = response.credits;
  const creditGroups = personCreditGroups(credits);
  const age = personAgeLabel(response.person.birthday, response.person.deathday);

  return `
    <section class="item-page person-page">
      <section class="item-hero person-hero">
        <div class="detail-art item-poster person-poster ${personImageUrl ? 'has-image' : ''}">
          ${personImageUrl ? `<img src="${escapeHtml(personImageUrl)}" alt="${escapeHtml(response.person.name)}" />` : `<span>${escapeHtml(response.person.name.slice(0, 1).toUpperCase())}</span>`}
        </div>
        <div class="detail-summary item-summary">
          <h2 class="item-title-fallback">${escapeHtml(response.person.name)}</h2>
          <div class="hero-meta-row">
            <span class="tag">${escapeHtml(providerDisplayName(response.person.provider_id))}</span>
            <span class="tag">${credits.length} item${credits.length === 1 ? '' : 's'}</span>
            ${response.person.birthday ? `<span class="tag">${escapeHtml([formatPersonDate(response.person.birthday), age].filter(Boolean).join(' · '))}</span>` : ''}
            ${response.person.gender ? `<span class="tag">${escapeHtml(response.person.gender)}</span>` : ''}
          </div>
          ${response.person.birth_place ? `<p class="hero-tagline">${escapeHtml(response.person.birth_place)}</p>` : ''}
          ${response.person.biography ? renderCollapsibleText(response.person.biography, `person-biography:${response.person.id}`) : ''}
          ${response.person.known_for.length ? `<div class="hero-meta-row">${response.person.known_for.map((title) => `<span class="tag">${escapeHtml(title)}</span>`).join('')}</div>` : ''}
          <div class="detail-actions">
            <button type="button" class="secondary-button" id="back-to-library">${renderButtonContent('Back', 'arrow-left')}</button>
            ${response.person.profile_url ? `<a class="button-link secondary-button" href="${escapeHtml(response.person.profile_url)}" target="_blank" rel="noreferrer">Provider page</a>` : ''}
          </div>
        </div>
      </section>

      <section class="panel page-panel item-section">
        <div class="section-heading section-heading-actions">
          <h3>Credits</h3>
          <span class="muted">${creditGroups.length} title${creditGroups.length === 1 ? '' : 's'}</span>
        </div>
        ${credits.length
          ? `<div class="person-credit-grid">${creditGroups.map(renderPersonCreditGroup).join('')}</div>`
          : '<div class="empty-state tight">No linked items are stored for this person yet.</div>'}
      </section>
    </section>
  `;
}

function directGridChildren(grid: HTMLElement, selector: string): HTMLElement[] {
  return Array.from(grid.children)
    .filter((child): child is HTMLElement => child instanceof HTMLElement && child.matches(selector));
}

function directGridChildByData(grid: HTMLElement, selector: string, key: string, value: string | undefined): HTMLElement | undefined {
  if (!value) {
    return undefined;
  }

  return directGridChildren(grid, selector).find((child) => child.dataset[key] === value);
}

function rowIndexForElement(element: HTMLElement, rowTops: number[]): number {
  const rowIndex = rowTops.findIndex((top) => Math.abs(top - element.offsetTop) < 8);
  return rowIndex >= 0 ? rowIndex : 0;
}

function activatePersonCreditTray(
  grid: HTMLElement,
  card: HTMLElement,
  tray: HTMLElement,
  cardSelector: string,
  traySelector: string,
): void {
  if (card.classList.contains('is-active') && tray.classList.contains('is-active')) {
    return;
  }

  const cards = directGridChildren(grid, cardSelector);
  const trays = directGridChildren(grid, traySelector);

  trays.forEach((entry) => {
    entry.classList.remove('is-active');
    entry.style.removeProperty('order');
  });
  cards.forEach((entry) => {
    entry.classList.remove('is-active');
    entry.style.removeProperty('order');
  });

  const rowTops = [...new Set(cards
    .map((entry) => entry.offsetTop)
    .sort((left, right) => left - right)
    .filter((top, index, values) => index === 0 || Math.abs(top - values[index - 1]) >= 8))];

  cards.forEach((entry) => {
    entry.style.order = String(rowIndexForElement(entry, rowTops) * 2);
  });

  card.classList.add('is-active');
  tray.classList.add('is-active');
  tray.style.order = String(rowIndexForElement(card, rowTops) * 2 + 1);
}

function collapsePersonCreditTrays(grid: HTMLElement, cardSelector: string, traySelector: string): void {
  directGridChildren(grid, cardSelector).forEach((entry) => {
    entry.classList.remove('is-active');
    entry.style.removeProperty('order');
  });
  directGridChildren(grid, traySelector).forEach((entry) => {
    entry.classList.remove('is-active');
    entry.style.removeProperty('order');
  });
}

function bindPersonCreditTrays(): void {
  const grid = document.querySelector<HTMLElement>('.person-credit-grid');
  if (!grid) {
    return;
  }

  const activateRootTray = (target: EventTarget | null): void => {
    const card = target instanceof Element ? target.closest<HTMLElement>('.person-credit-card') : null;
    if (!card || card.parentElement !== grid) {
      return;
    }

    const tray = directGridChildByData(grid, '.person-season-tray', 'personCreditId', card.dataset.personCreditId);
    if (!tray) {
      return;
    }

    activatePersonCreditTray(grid, card, tray, '.person-credit-card', '.person-season-tray');
  };

  const activateSeasonTray = (target: EventTarget | null): void => {
    const card = target instanceof Element ? target.closest<HTMLElement>('.person-season-credit-card') : null;
    const seasonGrid = card?.parentElement;
    if (!card || !(seasonGrid instanceof HTMLElement) || !seasonGrid.classList.contains('person-season-credit-grid')) {
      return;
    }

    const tray = directGridChildByData(seasonGrid, '.person-episode-tray', 'personSeasonCreditId', card.dataset.personSeasonCreditId);
    if (!tray) {
      return;
    }

    activatePersonCreditTray(seasonGrid, card, tray, '.person-season-credit-card', '.person-episode-tray');
  };

  grid.addEventListener('mouseover', (event) => {
    activateRootTray(event.target);
    activateSeasonTray(event.target);
  });
  grid.addEventListener('focusin', (event) => {
    activateRootTray(event.target);
    activateSeasonTray(event.target);
  });
  grid.addEventListener('click', (event) => {
    const target = event.target instanceof Element ? event.target : null;
    const rootCloseButton = target?.closest<HTMLButtonElement>('[data-close-person-credit-tray]');
    if (rootCloseButton) {
      event.preventDefault();
      event.stopPropagation();
      collapsePersonCreditTrays(grid, '.person-credit-card', '.person-season-tray');
      return;
    }

    const seasonCloseButton = target?.closest<HTMLButtonElement>('[data-close-person-season-credit-tray]');
    const seasonGrid = seasonCloseButton?.closest<HTMLElement>('.person-season-credit-grid');
    if (seasonCloseButton && seasonGrid) {
      event.preventDefault();
      event.stopPropagation();
      collapsePersonCreditTrays(seasonGrid, '.person-season-credit-card', '.person-episode-tray');
    }
  });
}

function renderItemPage(): string {
  if (!state.selectedItem) {
    return '<section class="panel page-panel"><div class="empty-state">Loading item details…</div></section>';
  }

  const posterUrl = state.selectedItem.poster_url
    ? getArtworkUrl(state.selectedItem.id, 'poster', state.selectedItem.artwork_updated_at)
    : undefined;
  const trailerOptions = currentTrailerOptions();
  const preferredTrailer = trailerOptions[0];
  const hasMultipleTrailers = trailerOptions.length > 1;
  const themeSongOption = currentThemeSongYouTubeTarget();
  const trailerButtonTitle = hasMultipleTrailers
    ? 'Click to play the first trailer. Right-click or press and hold to choose another trailer.'
    : 'Play Trailer';
  const playback = state.selectedPlayback;
  const library = state.libraries.find((entry) => entry.id === state.selectedItem?.library_id);
  const linkedMatch = state.selectedItemMetadata?.matches[0];
  const overview = state.selectedItem.overview
    ?? linkedMatch?.overview
    ?? 'No description is stored for this item yet.';
  const genres = state.selectedItem.genres.length
    ? state.selectedItem.genres
    : [];
  const logoUrl = state.selectedItem.logo_url ? resolveApiUrl(state.selectedItem.logo_url) : undefined;
  const technicalFacts = [
    { label: 'Duration', value: formatDuration(state.selectedItem.duration_ms) },
    {
      label: 'Format',
      value: [state.selectedItem.container?.toUpperCase(), state.selectedItem.media_kind.toUpperCase()].filter(Boolean).join(' • ') || 'Unknown',
    },
    {
      label: 'Codecs',
      value: [state.selectedItem.video_codec, state.selectedItem.audio_codec].filter(Boolean).join(' / ') || 'Unknown',
    },
    {
      label: 'Resolution',
      value: state.selectedItem.width && state.selectedItem.height ? `${state.selectedItem.width}×${state.selectedItem.height}` : 'Unknown',
    },
    { label: 'Bitrate', value: formatBitRate(state.selectedItem.bit_rate) },
    { label: 'Size', value: formatFileSize(state.selectedItem.file_size) },
  ];
  const hierarchy = state.selectedItem.hierarchy;
  const children = state.selectedItem.children;
  const backTarget = backNavigationTarget();
  const supportsManualLinking = canManuallyLinkMetadata(state.selectedItem);
  const metadataRefreshPending = itemIsMetadataPending(state.selectedItem);
  const resumeMs = resumablePlaybackPositionMs(state.selectedItem);
  const childSectionTitle = state.selectedItem.item_type === 'show'
    ? 'Seasons'
    : state.selectedItem.item_type === 'season'
      ? 'Episodes'
      : 'Contained items';
  return `
    <section class="item-page">
      ${hierarchy.length ? `
        <nav class="item-breadcrumbs panel page-panel" aria-label="Item hierarchy">
          ${hierarchy.map((item) => `
            <button type="button" class="breadcrumb-button" data-item-id="${item.id}">${escapeHtml(item.display_title)}</button>
          `).join('<span class="breadcrumb-separator">/</span>')}
          <span class="breadcrumb-separator">/</span>
          <span class="breadcrumb-current">${escapeHtml(state.selectedItem.display_title)}</span>
        </nav>
      ` : ''}
      <section class="item-hero ${state.selectedItem.item_type === 'episode' ? 'episode-hero' : ''}">
        <div class="detail-art item-poster ${state.selectedItem.item_type === 'episode' ? 'item-thumbnail' : ''} ${posterUrl ? 'has-image' : ''}">
          ${posterUrl ? `<img src="${escapeHtml(posterUrl)}" alt="${escapeHtml(state.selectedItem.display_title)} poster" />` : `<span>${escapeHtml(state.selectedItem.display_title.slice(0, 1).toUpperCase())}</span>`}
        </div>
        <div class="detail-summary item-summary">
          ${logoUrl
            ? `<img class="item-title-logo" src="${escapeHtml(logoUrl)}" alt="${escapeHtml(state.selectedItem.display_title)}" />`
            : `<h2 class="item-title-fallback">${escapeHtml(state.selectedItem.display_title)}</h2>`}
          ${state.selectedItem.tagline ? `<p class="hero-tagline">${escapeHtml(state.selectedItem.tagline)}</p>` : ''}
          <div class="hero-meta-row">
            ${state.selectedItem.release_year ? `<span class="tag">${state.selectedItem.release_year}</span>` : ''}
            ${state.selectedItem.content_rating ? `<span class="tag">${escapeHtml(state.selectedItem.content_rating)}</span>` : ''}
            ${typeof state.selectedItem.rating === 'number' ? `<span class="tag">${escapeHtml(state.selectedItem.rating.toFixed(1))}</span>` : ''}
            ${genres.map((genre) => `<span class="tag">${escapeHtml(genre)}</span>`).join('')}
          </div>
          ${renderCollapsibleText(overview, `item-overview:${state.selectedItem.id}`)}
          <div class="detail-actions">
            ${state.selectedItem.playable && resumeMs > 0 ? `<button type="button" data-play-selected-item-start-ms="${resumeMs}">${renderButtonContent(`Resume ${formatDuration(resumeMs)}`, 'play')}</button>` : ''}
            ${state.selectedItem.playable ? `<button type="button" class="${resumeMs > 0 ? 'secondary-button' : ''}" data-play-selected-item-start-ms="0">${renderButtonContent(resumeMs > 0 ? 'Start over' : 'Play now', 'play')}</button>` : ''}
            ${preferredTrailer ? `<button type="button" class="secondary-button" id="play-item-trailer" title="${escapeHtml(trailerButtonTitle)}">${renderButtonContent('Play Trailer', 'play')}</button>` : ''}
            ${themeSongOption ? `<button type="button" class="secondary-button" id="play-youtube-theme-song">${renderButtonContent('Play Theme', 'volume-2')}</button>` : ''}
            <button type="button" class="secondary-button" id="back-to-library">${renderButtonContent(backTarget.label, 'arrow-left')}</button>
          </div>
          ${hasMultipleTrailers && state.isTrailerMenuOpen ? `
            <section class="trailer-picker panel">
              <div class="section-heading section-heading-actions">
                <h3>Choose a trailer</h3>
                <button type="button" class="secondary-button" id="close-trailer-picker">${renderButtonContent('Close', 'x')}</button>
              </div>
              <div class="trailer-picker-list">
                ${trailerOptions.map((option, index) => `
                  <button type="button" class="secondary-button trailer-option-button" data-play-trailer-index="${index}">${escapeHtml(option.title)}</button>
                `).join('')}
              </div>
            </section>
          ` : ''}
          <p class="muted">${escapeHtml(playback?.reason ?? 'Loading playback capabilities…')}</p>
          <div class="item-fact-list">
            ${technicalFacts.map((fact) => `
              <div class="item-fact">
                <span class="label">${escapeHtml(fact.label)}</span>
                <strong>${escapeHtml(fact.value)}</strong>
              </div>
            `).join('')}
          </div>
        </div>
      </section>

      ${renderPeopleRail()}

      ${children.length ? `
        <section class="panel page-panel item-section">
          <div class="section-heading section-heading-actions">
            <h3>${escapeHtml(childSectionTitle)}</h3>
            <span class="muted">${children.length} item${children.length === 1 ? '' : 's'}</span>
          </div>
          <div class="item-grid hierarchy-item-grid ${state.selectedItem.item_type === 'season' ? 'season-episodes-grid' : ''}">${children.map(renderItemCard).join('')}</div>
        </section>
      ` : ''}

      ${renderSelectedItemCollectionRails()}

      <section class="item-support-grid">
        <section class="panel page-panel item-section">
          <div class="section-heading">
            <h3>File and library</h3>
          </div>
          <div class="item-info-list">
            <div>
              <span class="label">Library</span>
              <strong>${escapeHtml(library?.name ?? 'Unknown')}</strong>
            </div>
            <div>
              <span class="label">Folders</span>
              <strong>${escapeHtml(String(library?.paths.length ?? 0))}</strong>
            </div>
            <div>
              <span class="label">Source</span>
              <strong>${escapeHtml(state.selectedItem.relative_path)}</strong>
            </div>
            <div>
              <span class="label">Updated</span>
              <strong>${escapeHtml(formatTimestamp(state.selectedItem.modified_at))}</strong>
            </div>
          </div>
        </section>

        <section class="panel page-panel item-section item-link-panel">
          <div class="section-heading section-heading-actions">
            <h3>${supportsManualLinking ? 'Link metadata' : 'Metadata'}</h3>
            ${supportsManualLinking
              ? `<button type="button" class="secondary-button" id="refresh-item-metadata" ${linkedMatch && !metadataRefreshPending ? '' : 'disabled'}>${renderButtonContent(metadataRefreshPending ? 'Refreshing metadata' : 'Force refresh metadata', 'refresh-cw')}</button>`
              : ''}
          </div>
          ${renderLinkedMetadataSummary()}
          ${supportsManualLinking
            ? `
              <form id="metadata-search-form" class="metadata-search-form">
                <input id="metadata-search-input" name="metadataSearch" type="search" value="${escapeHtml(state.metadataSearchQuery)}" placeholder="${escapeHtml(selectedItemDefaultMetadataTitle() || 'Title')}" autocomplete="off" />
                <input id="metadata-search-year" name="metadataSearchYear" type="number" min="1800" max="2200" value="${escapeHtml(state.metadataSearchYear)}" placeholder="${escapeHtml(selectedItemDefaultMetadataYear() || 'Year')}" autocomplete="off" />
                <input id="metadata-search-language" name="metadataSearchLanguage" type="text" value="${escapeHtml(state.metadataSearchLanguage)}" placeholder="${escapeHtml(defaultMetadataSearchLanguage())}" autocomplete="off" />
                ${renderMetadataSearchProviderControls()}
                <button type="submit">${renderButtonContent('Search metadata', 'search')}</button>
              </form>
              <div class="metadata-search-list">${renderMetadataSearchResults()}</div>
            `
            : '<div class="empty-state tight">Season and episode metadata is inherited and refreshed automatically from the linked show.</div>'}
        </section>
      </section>
    </section>
  `;
}

const metadataLanguageOptions: Array<{ value: string; label: string }> = [
  { value: 'en-US', label: 'English (United States)' },
  { value: 'en-GB', label: 'English (United Kingdom)' },
  { value: 'es-ES', label: 'Spanish (Spain)' },
  { value: 'fr-FR', label: 'French (France)' },
  { value: 'de-DE', label: 'German (Germany)' },
  { value: 'it-IT', label: 'Italian (Italy)' },
  { value: 'ja-JP', label: 'Japanese (Japan)' },
  { value: 'pt-BR', label: 'Portuguese (Brazil)' },
];

function providerStatus(providerId: string): MetadataProviderStatus | undefined {
  return state.metadataProviders.find((provider) => provider.id === providerId);
}

function providerDisplayName(providerId: string): string {
  return providerStatus(providerId)?.display_name ?? providerId;
}

function libraryProviderOptions(libraryKind?: string): MetadataProviderStatus[] {
  return state.metadataProviders
    .filter((provider) => !libraryKind || provider.supported_kinds.includes(libraryKind));
}

function normalizedMetadataLanguages(languages?: string[]): string[] {
  const normalized = (languages ?? [])
    .map((language) => language.trim())
    .filter(Boolean)
    .filter((language, index, values) => values.indexOf(language) === index);
  return normalized.length ? normalized : ['en-US'];
}

function metadataLanguageSelect(name: string, selectedLanguages?: string[]): string {
  const selected = normalizedMetadataLanguages(selectedLanguages);
  return `
    <select name="${name}" multiple size="${Math.min(5, metadataLanguageOptions.length)}">
      ${metadataLanguageOptions.map((option) => `
        <option value="${option.value}" ${selected.includes(option.value) ? 'selected' : ''}>${escapeHtml(option.label)}</option>
      `).join('')}
    </select>
  `;
}

function metadataLanguageModeSelect(name: string, selectedMode?: 'auto' | 'manual'): string {
  const mode = selectedMode ?? 'auto';
  return `
    <select name="${name}">
      <option value="auto" ${mode === 'auto' ? 'selected' : ''}>Auto</option>
      <option value="manual" ${mode === 'manual' ? 'selected' : ''}>Manual</option>
    </select>
  `;
}

function userPermissionSelect(name: string, allowedUserIds?: number[]): string {
  const selected = new Set(allowedUserIds ?? []);
  return `
    <select name="${name}" multiple size="${Math.min(5, Math.max(2, state.users.length))}">
      ${state.users.map((user) => `
        <option value="${user.id}" ${selected.has(user.id) ? 'selected' : ''}>${escapeHtml(user.username)}${user.admin ? ' (admin)' : ''}</option>
      `).join('')}
    </select>
  `;
}

function metadataProviderCheckboxes(prefix: string, selectedProviders: string[], libraryKind?: string): string {
  const providers = libraryProviderOptions(libraryKind)
    .sort((left, right) => {
      const leftIndex = selectedProviders.indexOf(left.id);
      const rightIndex = selectedProviders.indexOf(right.id);
      return (left.role === right.role ? 0 : left.role === 'primary' ? -1 : 1)
        || (leftIndex < 0 ? Number.MAX_SAFE_INTEGER : leftIndex)
        - (rightIndex < 0 ? Number.MAX_SAFE_INTEGER : rightIndex)
        || left.display_name.localeCompare(right.display_name);
    });
  const selected = new Set(selectedProviders);
  let primaryPriority = 0;

  return `
    <div class="metadata-provider-list" data-provider-list="${prefix}">
      ${providers.map((provider) => `
      ${(() => {
        const providerId = provider.id;
        const label = provider.display_name;
        const isSecondary = provider.role === 'secondary';
        if (!isSecondary) {
          primaryPriority += 1;
        }
        const secondaryAvailable = !isSecondary
          || provider.extends_provider_ids.some((primaryProviderId) => selected.has(primaryProviderId));
        const checked = selected.has(providerId) && secondaryAvailable;
        return `
      <div class="metadata-provider-option" data-provider-option="${providerId}" data-provider-role="${provider.role}" data-extends-provider-ids="${provider.extends_provider_ids.join(',')}">
        <div class="provider-option-main">
          <label class="checkbox-inline">
            <input
              name="${prefix}"
              type="checkbox"
              value="${providerId}"
              data-provider-kinds="${provider.supported_kinds.join(',')}"
              ${checked ? 'checked' : ''}
              ${secondaryAvailable ? '' : 'disabled'}
            />
            ${escapeHtml(label)}
          </label>
          <span class="muted">${isSecondary ? 'Secondary' : `Priority ${primaryPriority}`}</span>
        </div>
        <div class="provider-option-actions">
          ${isSecondary ? '' : `
            <button type="button" class="secondary-button icon-only" data-provider-move="up" title="Move up" aria-label="Move ${escapeHtml(label)} up">${renderIcon('chevron-left')}</button>
            <button type="button" class="secondary-button icon-only" data-provider-move="down" title="Move down" aria-label="Move ${escapeHtml(label)} down">${renderIcon('chevron-right')}</button>
          `}
          <button type="button" class="secondary-button" data-provider-settings="${providerId}">${renderButtonContent('Settings', 'settings')}</button>
        </div>
      </div>
        `;
      })()}
      `).join('')}
    </div>
    `;
}

function renderExistingLibrariesSettings(settings: SettingsSnapshot): string {
  if (!settings.media.libraries.length) {
    return '<div class="empty-state tight">No libraries are configured yet.</div>';
  }

  return settings.media.libraries
    .map((library, index) => {
      const persistedLibrary = persistedLibraryForSettings(library);
      const refreshPending = persistedLibrary ? Boolean(libraryRefreshProgress(persistedLibrary)) : false;
      const refreshLabel = refreshPending
        ? 'Refreshing metadata'
        : 'Refresh metadata';

      return `
      <section class="settings-library-card">
        <div class="settings-library-header">
          <div>
            <p class="eyebrow">Library ${index + 1}</p>
            <h3>${escapeHtml(library.name || `Library ${index + 1}`)}</h3>
          </div>
          <div class="settings-library-actions">
            ${persistedLibrary
              ? `
                <button type="button" class="secondary-button" data-scan-library-id="${persistedLibrary.id}">${renderButtonContent('Scan now', 'refresh-cw')}</button>
                <button type="button" class="secondary-button" data-refresh-library-id="${persistedLibrary.id}" ${refreshPending ? 'disabled' : ''}>${renderButtonContent(refreshLabel, 'refresh-cw')}</button>
              `
              : ''}
            <button type="button" class="secondary-button danger-button" data-remove-library-index="${index}">${renderButtonContent('Remove library', 'trash-2')}</button>
          </div>
        </div>
        <div class="form-row">
          <label>Name<input name="existing_library_name_${index}" value="${escapeHtml(library.name)}" /></label>
          <label>Type
            <select name="existing_library_kind_${index}">
              ${libraryKindOptions(library.kind)}
            </select>
          </label>
        </div>
        <label>Folders
          <textarea name="existing_library_paths_${index}" rows="4" placeholder="One folder per line">${escapeHtml(joinPaths(library.paths.length ? library.paths : [library.path].filter(Boolean)))}</textarea>
        </label>
        <div class="form-row">
          <label class="checkbox-inline"><input name="existing_library_recursive_${index}" type="checkbox" ${library.recursive ? 'checked' : ''} /> Recursive scan</label>
        </div>
        <div class="form-row">
          <label>Provider language mode
            ${metadataLanguageModeSelect(`existing_library_metadata_language_mode_${index}`, library.metadata_language_mode)}
          </label>
          <label>Manual languages
            ${metadataLanguageSelect(`existing_library_metadata_language_${index}`, library.metadata_languages)}
          </label>
        </div>
        <div class="form-row">
          <label>Library access
            ${userPermissionSelect(`existing_library_allowed_user_${index}`, library.allowed_user_ids)}
          </label>
        </div>
        <fieldset>
          <legend>Metadata sources</legend>
          ${metadataProviderCheckboxes(`existing_library_metadata_provider_${index}`, library.metadata_providers, library.kind)}
        </fieldset>
      </section>
    `;
    })
    .join('');
}

function libraryKindOptions(selectedKind: string): string {
  return [
    ['movies', 'Movies'],
    ['shows', 'Shows'],
    ['music', 'Music'],
    ['photos', 'Photos'],
    ['books', 'Books'],
    ['home_videos', 'Home videos'],
  ]
    .map(([value, label]) => `<option value="${value}" ${selectedKind === value ? 'selected' : ''}>${label}</option>`)
    .join('');
}

function renderProviderSettingsCard(provider: MetadataProviderSettings): string {
  const label = providerDisplayName(provider.id);
  const status = state.metadataProviders.find((entry) => entry.id === provider.id);
  const logoUrl = status?.logo_dark_url ?? status?.logo_light_url;
  const showApiKey = Boolean(status?.requires_api_key);
  const showRequestSettings = provider.id !== 'local_nfo';
  return `
    <section class="settings-library-card provider-settings-card" id="provider-${escapeHtml(provider.id)}">
      <div class="settings-library-header">
        <div class="provider-settings-title">
          ${logoUrl ? `<img class="provider-settings-logo" src="${escapeHtml(logoUrl)}" alt="" />` : ''}
          <div>
          <p class="eyebrow">Provider</p>
          <h3>${escapeHtml(label)}</h3>
          </div>
        </div>
        ${status?.role ? `<span class="tag">${escapeHtml(status.role === 'secondary' ? 'Secondary' : 'Primary')}</span>` : ''}
      </div>
      ${status?.description ? `<p class="muted">${escapeHtml(status.description)}</p>` : ''}
      ${status?.attribution_text ? `<p class="muted">${escapeHtml(status.attribution_text)}</p>` : ''}
      ${showApiKey || showRequestSettings ? `<div class="form-row">
        ${showApiKey ? `<label>API key<input name="${provider.id}_api_key" value="${escapeHtml(provider.api_key ?? '')}" autocomplete="off" /></label>` : ''}
        ${showRequestSettings ? `
        <label>Rate limit (requests/second)<input name="${provider.id}_rate_limit_per_second" type="number" min="1" value="${provider.rate_limit_per_second}" /></label>
        <label>Retry attempts<input name="${provider.id}_retry_attempts" type="number" min="0" value="${provider.retry_attempts}" /></label>
        <label>Retry backoff (ms)<input name="${provider.id}_retry_backoff_ms" type="number" min="1" step="100" value="${provider.retry_backoff_ms}" /></label>
        ` : ''}
      </div>` : '<p class="muted">This provider does not require provider-specific settings.</p>'}
    </section>
  `;
}

function renderProviderSettingsPage(settings: SettingsSnapshot): string {
  return `
    <section class="panel page-panel settings-page-panel">
      <form id="settings-form" class="settings-form">
        <section>
          <div class="section-heading">
            <h3>Metadata providers</h3>
          </div>
          <p class="muted">Provider credentials and retry behavior are configured here. Metadata languages are selected per library.</p>
          <div class="settings-library-list">
            ${settings.metadata.providers.map(renderProviderSettingsCard).join('')}
          </div>
          <div class="form-row">
            <label>Automatic refresh
              <select name="metadata_refresh_interval_days">
                <option value="30" ${settings.metadata.refresh_interval_days === 30 ? 'selected' : ''}>Every 30 days</option>
                <option value="60" ${settings.metadata.refresh_interval_days === 60 ? 'selected' : ''}>Every 60 days</option>
                <option value="90" ${settings.metadata.refresh_interval_days === 90 ? 'selected' : ''}>Every 90 days</option>
                <option value="never" ${settings.metadata.refresh_interval_days == null ? 'selected' : ''}>Never</option>
              </select>
            </label>
          </div>
          <div class="form-row">
            <button type="button" class="secondary-button" id="clear-metadata-cache">${renderButtonContent('Clear metadata cache', 'trash-2')}</button>
            <p class="muted">Provider response cache is kept for 24 hours by default.</p>
          </div>
        </section>
        <div class="page-actions">
          <button type="submit">${renderButtonContent('Save provider settings', 'save')}</button>
        </div>
      </form>
    </section>
  `;
}

function renderSettingsPage(): string {
  const settings = state.settingsResponse?.settings;
  if (!settings) {
    return '<section class="panel page-panel"><div class="empty-state">Settings are still loading…</div></section>';
  }

  const section = activeSettingsSection();

  return `
    ${renderPageNavbar(
      'Settings',
      'Program configuration',
      `Saved to ${state.settingsResponse?.settings_path ?? ''}`,
    )}
    ${renderSettingsSectionNav()}
    ${section === 'general' ? `
    <section class="panel page-panel settings-page-panel">
      <form id="settings-form" class="settings-form">
        <section>
          <h3>Server</h3>
          <label>Data directory<input name="data_dir" value="${escapeHtml(settings.general.data_dir)}" /></label>
          <div class="form-row">
            <label>Address<input name="address" value="${escapeHtml(settings.server.address)}" /></label>
            <label>Port<input name="port" type="number" min="1" value="${settings.server.port}" /></label>
          </div>
          <div class="form-row checkbox-row">
            <label><input name="use_https" type="checkbox" ${settings.server.use_https ? 'checked' : ''} /> Use HTTPS</label>
            <label><input name="use_custom_certs" type="checkbox" ${settings.server.use_custom_certs ? 'checked' : ''} /> Use custom certificates</label>
          </div>
          <div class="form-row">
            <label>Certificate path<input name="cert_path" value="${escapeHtml(settings.server.cert_path)}" /></label>
            <label>Key path<input name="key_path" value="${escapeHtml(settings.server.key_path)}" /></label>
          </div>
        </section>

        <section>
          <h3>FFmpeg</h3>
          <div class="form-row">
            <label>ffmpeg path<input name="ffmpeg_path" value="${escapeHtml(settings.ffmpeg.ffmpeg_path)}" /></label>
            <label>ffprobe path<input name="ffprobe_path" value="${escapeHtml(settings.ffmpeg.ffprobe_path)}" /></label>
          </div>
        </section>

        <section>
          <h3>Metadata providers</h3>
          <p class="muted">Provider credentials and refresh behavior are configured on their own settings page.</p>
          <button type="button" class="secondary-button" data-settings-section-path="/settings/providers">${renderButtonContent('Open provider settings', 'settings')}</button>
        </section>

        <div class="page-actions">
          <button type="submit">${renderButtonContent('Save settings', 'save')}</button>
          <button type="button" class="secondary-button" id="go-home-from-settings">${renderButtonContent('Back home', 'house')}</button>
        </div>
      </form>

      ${renderUserManagement()}
    </section>
    ` : ''}
    ${section === 'providers' ? renderProviderSettingsPage(settings) : ''}
    ${section === 'libraries' ? `
      <section class="panel page-panel settings-page-panel">
        <form id="settings-form" class="settings-form">
          <section>
            <div class="section-heading">
              <h3>Libraries</h3>
            </div>
            <p class="muted">Each logical library can now contain multiple folders. Enter one folder per line.</p>
            <div class="settings-library-list">
              ${renderExistingLibrariesSettings(settings)}
            </div>
          </section>
          <div class="page-actions">
            <button type="submit">${renderButtonContent('Save library settings', 'save')}</button>
          </div>
        </form>

        <form id="add-library-form" class="settings-form add-library-form">
          <section>
            <h3>Add library</h3>
            <label>Name<input name="library_name" placeholder="Movies" required /></label>
            <label>Folders
              <textarea name="library_paths" rows="4" placeholder="C:/Media/Movies&#10;D:/Overflow/Movies" required></textarea>
            </label>
            <div class="form-row">
              <label>Type
                <select name="library_kind">
                  ${libraryKindOptions('movies')}
                </select>
              </label>
              <label class="checkbox-inline"><input name="library_recursive" type="checkbox" checked /> Recursive scan</label>
            </div>
            <div class="form-row">
              <label>Provider language mode
                ${metadataLanguageModeSelect('library_metadata_language_mode', 'auto')}
              </label>
              <label>Manual languages
                ${metadataLanguageSelect('library_metadata_language', ['en-US'])}
              </label>
            </div>
            <div class="form-row">
              <label>Library access
                ${userPermissionSelect('library_allowed_user', [])}
              </label>
            </div>
            <fieldset>
              <legend>Metadata sources</legend>
              <div id="add-library-metadata-providers">${metadataProviderCheckboxes('library_metadata_provider', ['tmdb'])}</div>
            </fieldset>
          </section>
          <button type="submit">${renderButtonContent('Add library', 'plus')}</button>
        </form>
      </section>
    ` : ''}
    ${section === 'dashboard' ? `
      <div id="metadata-dashboard-panel-root">${renderMetadataDashboard()}</div>
      <div id="system-activities-panel-root">${renderSystemActivitiesPanel()}</div>
    ` : ''}
    ${section === 'logs' ? `<div id="log-viewer-panel-root">${renderLogViewer()}</div>` : ''}
  `;
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

function renderPlayerOverlay(): string {
  if (state.activeTrailer) {
    const videoId = extractYouTubeVideoId(state.activeTrailer.url);
    const watchUrl = buildYouTubeWatchUrl(state.activeTrailer.url);
    const label = state.activeTrailer.label ?? 'Trailer';
    return `
      <div class="player-overlay trailer-overlay">
        <div class="player-shell trailer-shell is-controls-visible" tabindex="-1" ${videoId ? `data-trailer-video-id="${escapeHtml(videoId)}"` : ''}>
          <div class="trailer-frame-shell" aria-label="${escapeHtml(state.activeTrailer.title)}">
            ${videoId
              ? '<div id="trailer-player" class="trailer-youtube-player"></div>'
              : '<div class="trailer-unavailable">This trailer URL is not a controllable YouTube video.</div>'}
          </div>
          <div class="trailer-youtube-chrome-mask" aria-hidden="true"></div>
          <div class="player-loading-indicator" aria-live="polite">
            <span class="loading-spinner player-loading-spinner" aria-hidden="true"></span>
          </div>
          <div class="player-error-indicator" aria-live="polite">
            <strong>Trailer could not start</strong>
            <span>Open it on YouTube or try again in a moment.</span>
          </div>
          <div class="player-idle-hit-area trailer-idle-hit-area" aria-hidden="true"></div>
          <div class="player-top-controls player-controls trailer-top-controls">
            <div class="player-title-block">
              <span class="eyebrow">${escapeHtml(label)}</span>
              <h2>${escapeHtml(state.activeTrailer.title)}</h2>
            </div>
            <div class="player-top-actions">
              ${watchUrl ? `<a class="button-link secondary-button" href="${escapeHtml(watchUrl)}" target="_blank" rel="noreferrer">${renderButtonContent('Open on YouTube', 'arrow-right')}</a>` : ''}
              <button id="close-trailer" class="player-icon-button" type="button" title="Close trailer" aria-label="Close trailer">${renderIcon('x', 'player-control-icon')}</button>
            </div>
          </div>
          ${videoId ? `
            <div class="player-center-controls player-controls">
              <button class="player-icon-button player-large-button" type="button" data-trailer-seek="-10" title="Back 10 seconds" aria-label="Back 10 seconds">${renderIcon('skip-back', 'player-control-icon')}</button>
              <button id="trailer-play-toggle-large" class="player-icon-button player-primary-button" type="button" title="Pause" aria-label="Pause">${renderIcon('pause', 'player-control-icon')}</button>
              <button class="player-icon-button player-large-button" type="button" data-trailer-seek="30" title="Forward 30 seconds" aria-label="Forward 30 seconds">${renderIcon('skip-forward', 'player-control-icon')}</button>
            </div>
            <div class="player-bottom-controls player-controls">
              <input id="trailer-progress" class="player-progress" type="range" min="0" max="1000" value="0" step="1" aria-label="Trailer position" />
              <div class="player-control-row">
                <div class="player-control-cluster player-time-cluster">
                  <span class="player-time"><span id="trailer-current-time">0:00</span><span>/</span><span id="trailer-duration">0:00</span></span>
                </div>
                <div class="player-control-cluster player-transport-cluster">
                  <button class="player-icon-button" type="button" data-trailer-seek="-10" title="Back 10 seconds" aria-label="Back 10 seconds">${renderIcon('skip-back', 'player-control-icon')}</button>
                  <button id="trailer-play-toggle-small" class="player-icon-button player-primary-button" type="button" title="Pause" aria-label="Pause">${renderIcon('pause', 'player-control-icon')}</button>
                  <button class="player-icon-button" type="button" data-trailer-seek="30" title="Forward 30 seconds" aria-label="Forward 30 seconds">${renderIcon('skip-forward', 'player-control-icon')}</button>
                </div>
                <div class="player-control-cluster player-tool-cluster">
                  <button id="trailer-mute-toggle" class="player-icon-button" type="button" title="Mute" aria-label="Mute">${renderIcon('volume-2', 'player-control-icon')}</button>
                  <input id="trailer-volume" class="player-volume" type="range" min="0" max="1" value="${trailerMuted ? '0' : String(trailerVolume)}" step="0.01" aria-label="Trailer volume" />
                  <button id="trailer-fullscreen" class="player-icon-button" type="button" title="Fullscreen" aria-label="Fullscreen">${renderIcon('maximize', 'player-control-icon')}</button>
                </div>
              </div>
            </div>
          ` : ''}
        </div>
      </div>
    `;
  }

  if (!state.isPlayerOpen || !state.selectedItem || !state.activePlaybackSession) {
    return '';
  }

  const isAudio = state.selectedItem.media_kind === 'audio';
  const tag = isAudio ? 'audio' : 'video';
  const isExplicitAudioTrackSelection = state.activeAudioStreamIndex !== undefined;
  const selectedAudioStreamIndex = isExplicitAudioTrackSelection
    ? state.activeAudioStreamIndex
    : state.activePlaybackSession.audio_stream_index;
  const posterUrl = state.selectedItem.poster_url
    ? getArtworkUrl(state.selectedItem.id, 'poster', state.selectedItem.artwork_updated_at)
    : undefined;
  const backdropUrl = state.selectedItem.backdrop_url
    ? getArtworkUrl(state.selectedItem.id, 'backdrop', state.selectedItem.artwork_updated_at)
    : posterUrl;
  const logoUrl = state.selectedItem.logo_url ? resolveApiUrl(state.selectedItem.logo_url) : undefined;
  const trackMarkup = tag === 'video'
    ? state.selectedItem.subtitle_tracks
        .map((track) => `<track kind="subtitles" label="${escapeHtml(track.label)}" srclang="${escapeHtml(subtitleLanguage(track.label))}" src="${escapeHtml(resolveApiUrl(track.url))}" />`)
        .join('')
    : '';

  const isAudioStreamOverride = selectedAudioStreamIndex !== undefined && selectedAudioStreamIndex > 0;
  const isRemuxingForAudio = isAudioStreamOverride && !state.activePlaybackSession.decision.transcode_required;
  const streamStartMs = state.activePlaybackSession.decision.transcode_required || isRemuxingForAudio
    ? state.activePlaybackStartMs
    : 0;
  const source = getSessionStreamUrl(state.activePlaybackSession.session_id, streamStartMs, selectedAudioStreamIndex);
  const transcodeBadge = state.activePlaybackSession.decision.transcode_required || isRemuxingForAudio
    ? `<span class="player-badge is-transcoding" title="${escapeHtml(isRemuxingForAudio ? 'Using a non-default audio track requires a remuxed stream.' : state.activePlaybackSession.decision.reason)}">Transcoding</span>`
    : `<span class="player-badge is-direct" title="${escapeHtml(state.activePlaybackSession.decision.reason)}">Direct Play</span>`;
  const audioTracks = state.selectedItem.audio_tracks ?? [];
  const activeAudioTrack = audioTracks.find((track) => track.index === selectedAudioStreamIndex)
    ?? audioTracks.find((track) => track.default)
    ?? audioTracks[0];
  const audioTrackMenuTitle = activeAudioTrack
    ? `Audio track: ${activeAudioTrack.label}`
    : 'Audio track changes may require remuxing';

  return `
    <div class="player-overlay media-player-overlay">
      <div class="player-shell media-player-shell ${isAudio ? 'audio-player-shell' : 'video-player-shell'} is-controls-visible" tabindex="-1" ${backdropUrl ? `style="--player-backdrop-image: url('${escapeHtml(backdropUrl)}');"` : ''}>
        ${isAudio ? `
          <div class="audio-player-backdrop" aria-hidden="true"></div>
          <div class="audio-player-art ${posterUrl ? 'has-image' : ''}">
            ${posterUrl ? `<img src="${escapeHtml(posterUrl)}" alt="" />` : renderIcon('music', 'audio-player-art-icon')}
          </div>
          <audio id="media-player" autoplay preload="metadata" src="${escapeHtml(source)}"></audio>
        ` : `
          <video id="media-player" autoplay preload="metadata" playsinline src="${escapeHtml(source)}">${trackMarkup}</video>
        `}
        <div class="player-loading-indicator" aria-live="polite">
          <span class="loading-spinner player-loading-spinner" aria-hidden="true"></span>
        </div>
        <div class="player-error-indicator" aria-live="polite">
          <strong>Playback could not start</strong>
          <span>Try another audio track or start playback again.</span>
        </div>
        <div class="player-idle-hit-area" aria-hidden="true"></div>
        <div class="player-top-controls player-controls">
          <div class="player-title-block">
            <span class="eyebrow">Now playing</span>
            ${logoUrl
              ? `<img class="player-title-logo" src="${escapeHtml(logoUrl)}" alt="${escapeHtml(state.selectedItem.display_title)}" />`
              : `<h2>${escapeHtml(state.selectedItem.display_title)}</h2>`}
          </div>
          <div class="player-top-actions">
            ${transcodeBadge}
            <button id="close-player" class="player-icon-button" type="button" title="Close" aria-label="Close player">${renderIcon('x', 'player-control-icon')}</button>
          </div>
        </div>
        <div class="player-bottom-controls player-controls">
          <input id="player-progress" class="player-progress" type="range" min="0" max="1000" value="0" step="1" aria-label="Playback position" />
          <div class="player-control-row">
            <div class="player-control-cluster player-time-cluster">
              <span class="player-time"><span id="player-current-time">0:00</span><span>/</span><span id="player-duration">${escapeHtml(formatDuration(state.selectedItem.duration_ms))}</span></span>
            </div>
            <div class="player-control-cluster player-transport-cluster">
              <button class="player-icon-button" type="button" data-player-seek="-10" title="Back 10 seconds" aria-label="Back 10 seconds">${renderIcon('skip-back', 'player-control-icon')}</button>
              <button id="player-play-toggle-small" class="player-icon-button player-primary-button" type="button" title="Pause" aria-label="Pause">${renderIcon('pause', 'player-control-icon')}</button>
              <button class="player-icon-button" type="button" data-player-seek="10" title="Forward 10 seconds" aria-label="Forward 10 seconds">${renderIcon('skip-forward', 'player-control-icon')}</button>
            </div>
            <div class="player-control-cluster player-tool-cluster">
              <button id="player-mute-toggle" class="player-icon-button" type="button" title="Mute" aria-label="Mute">${renderIcon('volume-2', 'player-control-icon')}</button>
              <input id="player-volume" class="player-volume" type="range" min="0" max="1" value="1" step="0.01" aria-label="Volume" />
              ${!isAudio && audioTracks.length > 1 ? `
                <div class="player-menu-shell">
                  <button id="player-audio-track-toggle" class="player-icon-button" type="button" title="${escapeHtml(audioTrackMenuTitle)}" aria-label="Audio track" aria-expanded="${state.isAudioTrackMenuOpen ? 'true' : 'false'}" aria-haspopup="menu">${renderIcon('languages', 'player-control-icon')}</button>
                  <div id="player-audio-track-menu" class="player-track-menu ${state.isAudioTrackMenuOpen ? '' : 'is-hidden'}" role="menu" aria-label="Audio tracks" ${state.isAudioTrackMenuOpen ? '' : 'hidden'}>
                    ${audioTracks.map((track) => `
                      <button class="player-track-option ${track.index === activeAudioTrack?.index ? 'active' : ''}" type="button" role="menuitemradio" aria-checked="${track.index === activeAudioTrack?.index ? 'true' : 'false'}" data-player-audio-track-index="${track.index}">
                        <span>${escapeHtml(track.label)}</span>
                        <small>${escapeHtml([track.language?.toUpperCase(), track.codec?.toUpperCase()].filter(Boolean).join(' · ') || (track.default ? 'Default' : 'Audio'))}</small>
                      </button>
                    `).join('')}
                  </div>
                </div>
              ` : ''}
              ${isAudio ? '' : `<button id="player-pip" class="player-icon-button" type="button" title="Picture in picture" aria-label="Picture in picture">${renderIcon('picture-in-picture', 'player-control-icon')}</button>`}
              <button id="player-fullscreen" class="player-icon-button" type="button" title="Fullscreen" aria-label="Fullscreen">${renderIcon('maximize', 'player-control-icon')}</button>
            </div>
          </div>
        </div>
      </div>
    </div>
  `;
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
            <strong>${escapeHtml(currentUser()!.username)}</strong>
            <span>${currentUser()!.admin ? 'Administrator' : 'Signed in'}</span>
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

  const previousScrollTop = preserveScroll
    ? document.querySelector<HTMLElement>('.main-shell')?.scrollTop ?? 0
    : 0;
  const activeElement = document.activeElement as HTMLInputElement | null;
  const activeElementId = activeElement?.id;
  const activeSelection = activeElement
    && typeof activeElement.selectionStart === 'number'
    && typeof activeElement.selectionEnd === 'number'
      ? { start: activeElement.selectionStart, end: activeElement.selectionEnd }
      : undefined;

  if (!state.bootstrap && state.isLoading) {
    appRoot.innerHTML = renderAuthShell('Loading Koko', 'Checking server state and account access.', '');
    createIcons({ icons });
    return;
  }

  if (requiresSetup()) {
    appRoot.innerHTML = renderWelcomeScreen();
    createIcons({ icons });
    bindEvents();
    return;
  }

  if (requiresLogin()) {
    appRoot.innerHTML = renderLoginScreen();
    createIcons({ icons });
    bindEvents();
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

  appRoot.innerHTML = `
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
  `;

  createIcons({ icons });
  bindEvents();
  syncThemeSongPlayer();
  if (preserveScroll) {
    window.requestAnimationFrame(() => {
      const shell = document.querySelector<HTMLElement>('.main-shell');
      if (shell) {
        shell.scrollTop = previousScrollTop;
      }
      if (activeElementId) {
        const nextActiveElement = document.getElementById(activeElementId) as HTMLInputElement | null;
        nextActiveElement?.focus();
        if (activeSelection && nextActiveElement?.setSelectionRange) {
          nextActiveElement.setSelectionRange(activeSelection.start, activeSelection.end);
        }
      }
    });
  }
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
        ? searchItems(searchQuery, libraryId)
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

function setAuthFormBusy(form: HTMLFormElement, busy: boolean): void {
  form.querySelectorAll<HTMLInputElement | HTMLButtonElement>('input, button').forEach((control) => {
    control.disabled = busy;
  });
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
          ? searchItems(searchQuery, libraryId)
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

function buildSettingsFromForm(formData: FormData): SettingsSnapshot | undefined {
  const current = state.settingsResponse?.settings;
  if (!current) {
    return undefined;
  }
  const settingsSection = activeSettingsSection();

  return {
    general: {
      data_dir: String(formData.get('data_dir') ?? current.general.data_dir),
    },
    media: {
      libraries: current.media.libraries.map((library, index) => {
        const pathsField = `existing_library_paths_${index}`;
        if (!formData.has(pathsField)) {
          return library;
        }

        const paths = parsePathsInput(formData.get(pathsField));
        const providerField = `existing_library_metadata_provider_${index}`;
        return {
          name: String(formData.get(`existing_library_name_${index}`) ?? library.name),
          path: paths[0] ?? library.path,
          paths,
          recursive: formData.get(`existing_library_recursive_${index}`) === 'on',
          kind: String(formData.get(`existing_library_kind_${index}`) ?? library.kind),
          metadata_providers: formData.getAll(providerField).map((value) => String(value)),
          metadata_language_mode: String(formData.get(`existing_library_metadata_language_mode_${index}`) ?? library.metadata_language_mode ?? 'auto') === 'manual'
            ? 'manual'
            : 'auto',
          metadata_languages: formData.has(`existing_library_metadata_language_${index}`)
            ? normalizedMetadataLanguages(formData.getAll(`existing_library_metadata_language_${index}`).map((value) => String(value)))
            : normalizedMetadataLanguages(library.metadata_languages),
          allowed_user_ids: formData.has(`existing_library_allowed_user_${index}`)
            ? formData.getAll(`existing_library_allowed_user_${index}`)
                .map((value) => Number(value))
                .filter((value) => Number.isFinite(value) && value > 0)
            : library.allowed_user_ids,
        };
      }),
    },
    metadata: {
      refresh_interval_days: formData.has('metadata_refresh_interval_days')
        ? String(formData.get('metadata_refresh_interval_days') ?? '') === 'never'
          ? null
          : Number(formData.get('metadata_refresh_interval_days') ?? current.metadata.refresh_interval_days ?? 30)
        : current.metadata.refresh_interval_days,
      providers: current.metadata.providers.map((provider) => {
        const prefix = provider.id;
        if (
          !formData.has(`${prefix}_api_key`)
          && !formData.has(`${prefix}_rate_limit_per_second`)
          && !formData.has(`${prefix}_retry_attempts`)
          && !formData.has(`${prefix}_retry_backoff_ms`)
        ) {
          return provider;
        }

        return {
          ...provider,
          api_key: formData.has(`${prefix}_api_key`)
            ? String(formData.get(`${prefix}_api_key`) ?? '')
            : provider.api_key,
          rate_limit_per_second: Math.max(1, Number(formData.get(`${prefix}_rate_limit_per_second`) ?? provider.rate_limit_per_second)),
          retry_attempts: Math.max(0, Number(formData.get(`${prefix}_retry_attempts`) ?? provider.retry_attempts)),
          retry_backoff_ms: Math.max(1, Number(formData.get(`${prefix}_retry_backoff_ms`) ?? provider.retry_backoff_ms)),
        };
      }),
    },
    server: {
      use_https: settingsSection === 'general' ? formData.get('use_https') === 'on' : current.server.use_https,
      address: String(formData.get('address') ?? current.server.address),
      port: Number(formData.get('port') ?? current.server.port),
      cert_path: String(formData.get('cert_path') ?? current.server.cert_path),
      key_path: String(formData.get('key_path') ?? current.server.key_path),
      use_custom_certs: settingsSection === 'general'
        ? formData.get('use_custom_certs') === 'on'
        : current.server.use_custom_certs,
    },
    ffmpeg: {
      ffmpeg_path: String(formData.get('ffmpeg_path') ?? current.ffmpeg.ffmpeg_path),
      ffprobe_path: String(formData.get('ffprobe_path') ?? current.ffmpeg.ffprobe_path),
    },
  };
}

function themeSongLayer(): HTMLElement {
  let layer = document.querySelector<HTMLElement>('#theme-song-layer');
  if (!layer) {
    layer = document.createElement('div');
    layer.id = 'theme-song-layer';
    document.body.appendChild(layer);
  }

  return layer;
}

function currentThemeSongSource(): ThemeSongSource | undefined {
  if (state.isPlayerOpen || state.activeTrailer) {
    return undefined;
  }

  const target = currentThemeSongTarget();
  return target ? themeSongSourceFromUrl(target.url, target.title) : undefined;
}

function themeSongSourceFromUrl(
  themeSongUrl: string,
  title: string,
): ThemeSongSource | undefined {
  if (!themeSongUrl) {
    return undefined;
  }

  const videoId = extractYouTubeVideoId(themeSongUrl);
  if (videoId) {
    return {
      kind: 'youtube',
      src: videoId,
      title,
      videoId,
    };
  }

  return {
    kind: 'audio',
    src: resolveApiUrl(themeSongUrl),
    title,
  };
}

function loadYouTubeIframeApi(): Promise<YouTubeIframeApi> {
  if (window.YT?.Player) {
    return Promise.resolve(window.YT);
  }

  if (youtubeIframeApiPromise) {
    return youtubeIframeApiPromise;
  }

  youtubeIframeApiPromise = new Promise((resolve) => {
    const existingReadyHandler = window.onYouTubeIframeAPIReady;
    window.onYouTubeIframeAPIReady = () => {
      existingReadyHandler?.();
      if (window.YT?.Player) {
        resolve(window.YT);
      }
    };

    if (!document.querySelector<HTMLScriptElement>('script[src="https://www.youtube.com/iframe_api"]')) {
      const script = document.createElement('script');
      script.src = 'https://www.youtube.com/iframe_api';
      const firstScript = document.getElementsByTagName('script')[0];
      firstScript.parentNode?.insertBefore(script, firstScript);
    }
  });

  return youtubeIframeApiPromise;
}

function clearTrailerProgressHandle(): void {
  if (trailerProgressHandle !== undefined) {
    window.clearInterval(trailerProgressHandle);
    trailerProgressHandle = undefined;
  }
}

function destroyTrailerYouTubePlayer(): void {
  clearTrailerProgressHandle();
  trailerYouTubePlayerReady = undefined;
  if (!trailerYouTubePlayer) {
    activeTrailerYouTubeVideoId = undefined;
    document.body.style.cursor = '';
    return;
  }

  try {
    trailerYouTubePlayer.pauseVideo();
    trailerYouTubePlayer.destroy();
  } catch {
    // The trailer iframe may already have been removed during a render.
  } finally {
    trailerYouTubePlayer = undefined;
    activeTrailerYouTubeVideoId = undefined;
    document.body.style.cursor = '';
  }
}

function destroyThemeSongYouTubePlayer(): void {
  themeSongYouTubePlayerReady = undefined;
  if (!themeSongYouTubePlayer) {
    return;
  }

  try {
    themeSongYouTubePlayer.pauseVideo();
    themeSongYouTubePlayer.destroy();
  } catch {
    // The YouTube iframe may already have been removed during a render.
  } finally {
    themeSongYouTubePlayer = undefined;
    activeThemeSongYouTubeVideoId = undefined;
  }
}

function ensureThemeSongYouTubePlayer(): Promise<YouTubePlayer> {
  if (themeSongYouTubePlayer) {
    return Promise.resolve(themeSongYouTubePlayer);
  }

  if (themeSongYouTubePlayerReady) {
    return themeSongYouTubePlayerReady;
  }

  const layer = themeSongLayer();
  if (!document.querySelector('#theme-song-youtube-player')) {
    layer.innerHTML = '<div id="theme-song-youtube-player" class="theme-song-iframe"></div>';
  }

  themeSongYouTubePlayerReady = loadYouTubeIframeApi().then((api) => new Promise<YouTubePlayer>((resolve) => {
    themeSongYouTubePlayer = new api.Player('theme-song-youtube-player', {
      height: '0',
      width: '0',
      videoId: YOUTUBE_THEME_PLACEHOLDER_VIDEO_ID,
      playerVars: {
        autoplay: 0,
        controls: 2,
        loop: 0,
      },
      events: {
        onReady: (event) => {
          event.target.setPlaybackQuality('small');
          resolve(event.target);
        },
        onStateChange: () => {
          if (state.hasDeferredAutoRefreshRender) {
            state.hasDeferredAutoRefreshRender = false;
            render();
          }
        },
        onError: (event) => {
          console.warn('YouTube theme song playback failed', {
            videoId: activeThemeSongYouTubeVideoId,
            errorCode: event.data,
          });
        },
      },
    });
  }));

  return themeSongYouTubePlayerReady;
}

function playYouTubeThemeSong(videoId: string): void {
  activeThemeSongYouTubeVideoId = videoId;
  if (themeSongYouTubePlayer) {
    themeSongYouTubePlayer.loadVideoById(videoId);
    return;
  }

  void ensureThemeSongYouTubePlayer().then((player) => {
    player.loadVideoById(videoId);
  });
}

function syncThemeSongPlayer(): void {
  const layer = themeSongLayer();
  const source = currentThemeSongSource();
  if (!source) {
    destroyThemeSongYouTubePlayer();
    layer.replaceChildren();
    delete layer.dataset.themeKind;
    delete layer.dataset.themeSrc;
    return;
  }

  if (layer.hasChildNodes() && layer.dataset.themeKind === source.kind && layer.dataset.themeSrc === source.src) {
    return;
  }

  layer.dataset.themeKind = source.kind;
  layer.dataset.themeSrc = source.src;
  if (source.kind === 'youtube') {
    if (!document.querySelector('#theme-song-youtube-player')) {
      layer.innerHTML = '<div id="theme-song-youtube-player" class="theme-song-iframe"></div>';
    }
    playYouTubeThemeSong(source.videoId);
    return;
  }

  destroyThemeSongYouTubePlayer();
  layer.innerHTML = `<audio id="theme-song-player" autoplay preload="auto" src="${escapeHtml(source.src)}"></audio>`;
  const themePlayer = layer.querySelector<HTMLAudioElement>('#theme-song-player');
  if (!themePlayer) {
    return;
  }

  themePlayer.volume = 0.45;
  themePlayer.loop = false;
  themePlayer.addEventListener('ended', () => {
    if (state.hasDeferredAutoRefreshRender) {
      state.hasDeferredAutoRefreshRender = false;
      render();
    }
  }, { once: true });
  void themePlayer.play().catch(() => {
    // Autoplay can be blocked by the browser, so the page quietly falls back without looping.
  });
}

async function refreshLogsView(): Promise<void> {
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
    root.innerHTML = renderLogViewer();
    createIcons({ icons });
    bindEvents();
  }
}

function closeActivePlaybackSession(): void {
  state.isPlayerOpen = false;
  document.body.style.cursor = '';
  const sessionToClose = state.activePlaybackSession;
  state.activePlaybackSession = undefined;
  state.activePlaybackStartMs = 0;
  state.activeAudioStreamIndex = undefined;
  state.isAudioTrackMenuOpen = false;
  render();
  if (sessionToClose) {
    deletePlaybackSession(sessionToClose.session_id).catch((error) => {
      console.error('Failed to close playback session', error);
    });
  }
}

function ensureTrailerYouTubePlayer(videoId: string): Promise<YouTubePlayer> {
  if (trailerYouTubePlayer && activeTrailerYouTubeVideoId === videoId) {
    return Promise.resolve(trailerYouTubePlayer);
  }

  if (trailerYouTubePlayerReady && activeTrailerYouTubeVideoId === videoId) {
    return trailerYouTubePlayerReady;
  }

  destroyTrailerYouTubePlayer();
  activeTrailerYouTubeVideoId = videoId;
  const playerVars: Record<string, number | string> = {
    autoplay: 1,
    controls: 0,
    disablekb: 1,
    fs: 0,
    iv_load_policy: 3,
    loop: 0,
    modestbranding: 1,
    playsinline: 1,
    rel: 0,
  };
  if (window.location.origin.startsWith('http')) {
    playerVars.origin = window.location.origin;
  }

  trailerYouTubePlayerReady = loadYouTubeIframeApi().then((api) => new Promise<YouTubePlayer>((resolve) => {
    trailerYouTubePlayer = new api.Player('trailer-player', {
      height: '100%',
      width: '100%',
      videoId,
      playerVars,
      events: {
        onReady: (event) => {
          trailerYouTubePlayer = event.target;
          event.target.setPlaybackQuality('hd720');
          event.target.setVolume(Math.round(trailerVolume * 100));
          if (trailerMuted) {
            event.target.mute();
          } else {
            event.target.unMute();
          }
          event.target.playVideo();
          resolve(event.target);
        },
        onStateChange: () => {
          updateTrailerPlayerUi();
        },
        onError: (event) => {
          document.querySelector<HTMLElement>('.trailer-shell')?.classList.add('has-media-error');
          document.querySelector<HTMLElement>('.trailer-shell')?.classList.remove('is-media-loading');
          console.warn('YouTube trailer playback failed', {
            videoId: activeTrailerYouTubeVideoId,
            errorCode: event.data,
          });
        },
      },
    });
  }));

  return trailerYouTubePlayerReady;
}

function trailerPlayerState(): number | undefined {
  try {
    return trailerYouTubePlayer?.getPlayerState();
  } catch {
    return undefined;
  }
}

function isTrailerPlaying(): boolean {
  return trailerPlayerState() === YOUTUBE_PLAYER_STATE.playing;
}

function updateIconButton(
  button: HTMLButtonElement | null | undefined,
  iconName: AppIconName,
  label: string,
): void {
  if (!button) {
    return;
  }
  button.innerHTML = renderIcon(iconName, 'player-control-icon');
  button.title = label;
  button.setAttribute('aria-label', label);
  createIcons({ icons });
}

function updateTrailerPlayerUi(): void {
  const player = trailerYouTubePlayer;
  if (!player) {
    return;
  }

  const shell = document.querySelector<HTMLElement>('.trailer-shell');
  const progress = document.querySelector<HTMLInputElement>('#trailer-progress');
  const currentTimeLabel = document.querySelector<HTMLElement>('#trailer-current-time');
  const durationLabel = document.querySelector<HTMLElement>('#trailer-duration');
  const playButtons = Array.from(document.querySelectorAll<HTMLButtonElement>('#trailer-play-toggle-small, #trailer-play-toggle-large'));
  const muteButton = document.querySelector<HTMLButtonElement>('#trailer-mute-toggle');
  const volume = document.querySelector<HTMLInputElement>('#trailer-volume');
  const playerState = trailerPlayerState();
  const isPlaying = playerState === YOUTUBE_PLAYER_STATE.playing;
  const isLoading = playerState === YOUTUBE_PLAYER_STATE.buffering || playerState === YOUTUBE_PLAYER_STATE.cued;
  const duration = player.getDuration();
  const currentTime = player.getCurrentTime();

  shell?.classList.toggle('is-media-loading', isLoading);
  playButtons.forEach((button) => updateIconButton(button, isPlaying ? 'pause' : 'play', isPlaying ? 'Pause' : 'Play'));
  trailerMuted = player.isMuted() || player.getVolume() === 0;
  trailerVolume = Math.max(0, Math.min(1, player.getVolume() / 100));
  updateIconButton(muteButton, trailerMuted ? 'volume-x' : 'volume-2', trailerMuted ? 'Unmute' : 'Mute');
  if (volume && document.activeElement !== volume) {
    volume.value = String(trailerMuted ? 0 : trailerVolume);
  }
  if (progress && progress.dataset.scrubbing !== 'true') {
    progress.value = duration > 0 ? String(Math.min(1000, Math.max(0, (currentTime / duration) * 1000))) : '0';
  }
  if (currentTimeLabel) {
    currentTimeLabel.textContent = formatMediaTime(currentTime);
  }
  if (durationLabel) {
    durationLabel.textContent = formatMediaTime(duration);
  }
}

function bindTrailerPlayer(): void {
  if (!state.activeTrailer) {
    destroyTrailerYouTubePlayer();
    return;
  }

  const shell = document.querySelector<HTMLElement>('.trailer-shell');
  const videoId = shell?.dataset.trailerVideoId;
  if (!shell || !videoId) {
    return;
  }

  const progress = document.querySelector<HTMLInputElement>('#trailer-progress');
  const volume = document.querySelector<HTMLInputElement>('#trailer-volume');
  const currentTimeLabel = document.querySelector<HTMLElement>('#trailer-current-time');
  const playButtons = Array.from(document.querySelectorAll<HTMLButtonElement>('#trailer-play-toggle-small, #trailer-play-toggle-large'));
  const muteButton = document.querySelector<HTMLButtonElement>('#trailer-mute-toggle');
  const fullscreenButton = document.querySelector<HTMLButtonElement>('#trailer-fullscreen');
  const idleHitArea = document.querySelector<HTMLElement>('.trailer-idle-hit-area');
  let controlsHideHandle: number | undefined;
  let isScrubbing = false;

  const showControls = (): void => {
    shell.classList.add('is-controls-visible');
    shell.classList.remove('is-controls-hidden');
    document.body.style.cursor = '';
    if (controlsHideHandle !== undefined) {
      window.clearTimeout(controlsHideHandle);
    }
    controlsHideHandle = window.setTimeout(() => {
      if (isTrailerPlaying() && !isScrubbing) {
        shell.classList.remove('is-controls-visible');
        shell.classList.add('is-controls-hidden');
        document.body.style.cursor = 'none';
      }
    }, 3200);
  };

  const seekBy = (seconds: number): void => {
    if (!trailerYouTubePlayer) {
      return;
    }
    const duration = trailerYouTubePlayer.getDuration();
    const currentTime = trailerYouTubePlayer.getCurrentTime();
    const targetTime = duration > 0
      ? Math.min(duration, Math.max(0, currentTime + seconds))
      : Math.max(0, currentTime + seconds);
    trailerYouTubePlayer.seekTo(targetTime, true);
    updateTrailerPlayerUi();
  };

  const togglePlayback = (): void => {
    if (!trailerYouTubePlayer) {
      return;
    }
    if (isTrailerPlaying()) {
      trailerYouTubePlayer.pauseVideo();
    } else {
      trailerYouTubePlayer.playVideo();
    }
    updateTrailerPlayerUi();
  };

  const toggleFullscreen = (): void => {
    if (document.fullscreenElement) {
      void document.exitFullscreen();
      return;
    }
    void shell.requestFullscreen?.();
  };

  shell.focus({ preventScroll: true });
  ['mousemove', 'mousedown', 'touchstart', 'pointermove'].forEach((eventName) => {
    shell.addEventListener(eventName, showControls, { passive: true });
  });
  shell.addEventListener('keydown', (event) => {
    if (event.target instanceof HTMLInputElement) {
      return;
    }
    if (event.key === ' ' || event.key === 'k') {
      event.preventDefault();
      togglePlayback();
    } else if (event.key === 'ArrowLeft') {
      event.preventDefault();
      seekBy(-10);
    } else if (event.key === 'ArrowRight') {
      event.preventDefault();
      seekBy(30);
    } else if (event.key === 'm') {
      event.preventDefault();
      muteButton?.click();
    } else if (event.key === 'f') {
      event.preventDefault();
      toggleFullscreen();
    } else if (event.key === 'Escape') {
      event.preventDefault();
      closeTrailerPlayer();
    }
    showControls();
  });

  idleHitArea?.addEventListener('click', () => {
    togglePlayback();
    showControls();
  });
  playButtons.forEach((button) => button.addEventListener('click', () => {
    togglePlayback();
    showControls();
  }));
  document.querySelectorAll<HTMLButtonElement>('[data-trailer-seek]').forEach((button) => {
    button.addEventListener('click', () => {
      seekBy(Number(button.dataset.trailerSeek) || 0);
      showControls();
    });
  });
  muteButton?.addEventListener('click', () => {
    if (!trailerYouTubePlayer) {
      return;
    }
    if (trailerYouTubePlayer.isMuted() || trailerYouTubePlayer.getVolume() === 0) {
      trailerYouTubePlayer.unMute();
      if (trailerYouTubePlayer.getVolume() === 0) {
        trailerYouTubePlayer.setVolume(Math.round(Math.max(trailerVolume, 0.45) * 100));
      }
    } else {
      trailerYouTubePlayer.mute();
    }
    updateTrailerPlayerUi();
    showControls();
  });
  volume?.addEventListener('input', () => {
    if (!trailerYouTubePlayer) {
      return;
    }
    const nextVolume = Math.min(1, Math.max(0, Number(volume.value)));
    trailerVolume = nextVolume;
    trailerYouTubePlayer.setVolume(Math.round(nextVolume * 100));
    if (nextVolume <= 0) {
      trailerYouTubePlayer.mute();
    } else {
      trailerYouTubePlayer.unMute();
    }
    updateTrailerPlayerUi();
    showControls();
  });
  volume?.addEventListener('wheel', (event) => {
    event.preventDefault();
    if (!trailerYouTubePlayer) {
      return;
    }
    const delta = event.deltaY < 0 ? 0.05 : -0.05;
    const nextVolume = Math.min(1, Math.max(0, trailerVolume + delta));
    trailerVolume = nextVolume;
    trailerYouTubePlayer.setVolume(Math.round(nextVolume * 100));
    if (nextVolume <= 0) {
      trailerYouTubePlayer.mute();
    } else {
      trailerYouTubePlayer.unMute();
    }
    updateTrailerPlayerUi();
    showControls();
  }, { passive: false });
  fullscreenButton?.addEventListener('click', () => {
    toggleFullscreen();
    showControls();
  });
  progress?.addEventListener('input', () => {
    isScrubbing = true;
    progress.dataset.scrubbing = 'true';
    if (!trailerYouTubePlayer) {
      return;
    }
    const duration = trailerYouTubePlayer.getDuration();
    if (duration > 0 && currentTimeLabel) {
      currentTimeLabel.textContent = formatMediaTime((Number(progress.value) / 1000) * duration);
    }
    showControls();
  });
  progress?.addEventListener('wheel', (event) => {
    event.preventDefault();
    seekBy(event.deltaY < 0 ? 10 : -10);
    showControls();
  }, { passive: false });
  progress?.addEventListener('change', () => {
    if (trailerYouTubePlayer) {
      const duration = trailerYouTubePlayer.getDuration();
      if (duration > 0) {
        trailerYouTubePlayer.seekTo((Number(progress.value) / 1000) * duration, true);
      }
    }
    isScrubbing = false;
    delete progress.dataset.scrubbing;
    updateTrailerPlayerUi();
    showControls();
  });

  shell.classList.add('is-media-loading');
  void ensureTrailerYouTubePlayer(videoId).then((player) => {
    player.playVideo();
    updateTrailerPlayerUi();
    clearTrailerProgressHandle();
    trailerProgressHandle = window.setInterval(updateTrailerPlayerUi, 500);
    showControls();
  });
}

function bindPlayerProgress(): void {
  const player = document.querySelector<HTMLMediaElement>('#media-player');
  if (!player || !state.selectedItem) {
    return;
  }

  const shell = document.querySelector<HTMLElement>('.media-player-shell');
  const progress = document.querySelector<HTMLInputElement>('#player-progress');
  const volume = document.querySelector<HTMLInputElement>('#player-volume');
  const currentTimeLabel = document.querySelector<HTMLElement>('#player-current-time');
  const durationLabel = document.querySelector<HTMLElement>('#player-duration');
  const playButtons = Array.from(document.querySelectorAll<HTMLButtonElement>('#player-play-toggle-small'));
  const muteButton = document.querySelector<HTMLButtonElement>('#player-mute-toggle');
  const fullscreenButton = document.querySelector<HTMLButtonElement>('#player-fullscreen');
  const pipButton = document.querySelector<HTMLButtonElement>('#player-pip');
  const audioTrackToggle = document.querySelector<HTMLButtonElement>('#player-audio-track-toggle');
  const audioTrackMenu = document.querySelector<HTMLElement>('#player-audio-track-menu');
  const selectedAudioStreamIndex = state.activeAudioStreamIndex ?? state.activePlaybackSession?.audio_stream_index;
  const currentAudioTrackIndex = selectedAudioStreamIndex ?? 0;
  const isAudioStreamOverride = selectedAudioStreamIndex !== undefined && selectedAudioStreamIndex > 0;
  const isTranscoding = (state.activePlaybackSession?.decision.transcode_required ?? false) || isAudioStreamOverride;
  const sourceDurationSeconds = (state.selectedItem.duration_ms ?? 0) / 1000;
  const requestedPlaybackStartSeconds = Math.max(0, state.activePlaybackStartMs / 1000);
  const playbackBaseOffsetSeconds = isTranscoding ? requestedPlaybackStartSeconds : 0;
  const initialDirectSeekSeconds = isTranscoding ? 0 : requestedPlaybackStartSeconds;
  const skipSteps = [10, 20, 30, 60, 120, 300];
  let controlsHideHandle: number | undefined;
  let isScrubbing = false;
  let lastSkipDirection = 0;
  let lastSkipAt = 0;
  let skipStepIndex = 0;
  let hasAppliedInitialDirectSeek = initialDirectSeekSeconds <= 0;

  const setPlayerLoading = (loading: boolean): void => {
    const shouldShowLoading = loading && !player.ended && player.readyState < player.HAVE_FUTURE_DATA;
    shell?.classList.toggle('is-media-loading', shouldShowLoading);
    shell?.classList.remove('has-media-error');
  };

  const refreshPlayerLoading = (): void => {
    setPlayerLoading(!player.paused && player.readyState < player.HAVE_FUTURE_DATA);
  };

  const setPlayerError = (): void => {
    shell?.classList.remove('is-media-loading');
    shell?.classList.add('has-media-error');
  };

  const setButtonIcon = (button: HTMLButtonElement | null | undefined, iconName: AppIconName, label: string): void => {
    if (!button) {
      return;
    }
    button.innerHTML = renderIcon(iconName, 'player-control-icon');
    button.title = label;
    button.setAttribute('aria-label', label);
    createIcons({ icons });
  };

  const updatePlayButtons = (): void => {
    const iconName: AppIconName = player.paused ? 'play' : 'pause';
    const label = player.paused ? 'Play' : 'Pause';
    playButtons.forEach((button) => setButtonIcon(button, iconName, label));
  };

  const updateMuteButton = (): void => {
    setButtonIcon(muteButton, player.muted || player.volume === 0 ? 'volume-x' : 'volume-2', player.muted ? 'Unmute' : 'Mute');
    if (volume && !isScrubbing) {
      volume.value = String(player.muted ? 0 : player.volume);
    }
  };

  const updatePipButton = (): void => {
    if (!pipButton || !(player instanceof HTMLVideoElement)) {
      return;
    }
    const isSupported = document.pictureInPictureEnabled && !player.disablePictureInPicture;
    pipButton.disabled = !isSupported;
    pipButton.title = isSupported ? 'Picture in picture' : 'Picture in picture is not available in this browser';
    pipButton.setAttribute('aria-label', pipButton.title);
  };

  const setAudioTrackMenuOpen = (open: boolean): void => {
    state.isAudioTrackMenuOpen = open;
    audioTrackToggle?.setAttribute('aria-expanded', open ? 'true' : 'false');
    audioTrackMenu?.classList.toggle('is-hidden', !open);
    audioTrackMenu?.toggleAttribute('hidden', !open);
  };

  const updateTimeline = (): void => {
    const duration = sourceDurationSeconds > 0
      ? sourceDurationSeconds
      : Number.isFinite(player.duration) && player.duration > 0
        ? player.duration
        : 0;
    const currentPosition = Math.min(duration || Number.POSITIVE_INFINITY, playbackBaseOffsetSeconds + player.currentTime);
    if (progress && !isScrubbing) {
      progress.value = duration > 0 ? String(Math.min(1000, Math.max(0, (currentPosition / duration) * 1000))) : '0';
    }
    if (currentTimeLabel) {
      currentTimeLabel.textContent = formatMediaTime(currentPosition);
    }
    if (durationLabel) {
      durationLabel.textContent = formatMediaTime(duration);
    }
  };

  const applyInitialDirectSeek = (): void => {
    if (hasAppliedInitialDirectSeek || initialDirectSeekSeconds <= 0 || player.readyState < player.HAVE_METADATA) {
      return;
    }

    const duration = sourceDurationSeconds > 0
      ? sourceDurationSeconds
      : Number.isFinite(player.duration) && player.duration > 0
        ? player.duration
        : 0;
    const targetPosition = duration > 0
      ? Math.min(initialDirectSeekSeconds, Math.max(0, duration - 1))
      : initialDirectSeekSeconds;

    try {
      player.currentTime = targetPosition;
      hasAppliedInitialDirectSeek = true;
      updateTimeline();
    } catch (error) {
      console.warn('Failed to seek direct-play item to resume position', error);
    }
  };

  const showControls = (): void => {
    shell?.classList.add('is-controls-visible');
    shell?.classList.remove('is-controls-hidden');
    document.body.style.cursor = '';
    if (controlsHideHandle !== undefined) {
      window.clearTimeout(controlsHideHandle);
    }
    controlsHideHandle = window.setTimeout(() => {
      if (!player.paused && !isScrubbing) {
        shell?.classList.remove('is-controls-visible');
        shell?.classList.add('is-controls-hidden');
        document.body.style.cursor = 'none';
      }
    }, 3200);
  };

  const seekWithEscalation = (direction: number): void => {
    const now = Date.now();
    if (direction !== 0 && direction === lastSkipDirection && now - lastSkipAt < 900) {
      skipStepIndex = Math.min(skipSteps.length - 1, skipStepIndex + 1);
    } else {
      skipStepIndex = 0;
    }
    lastSkipDirection = direction;
    lastSkipAt = now;
    seekBy(direction * skipSteps[skipStepIndex]);
  };

  const seekBy = (seconds: number): void => {
    const currentPosition = playbackBaseOffsetSeconds + player.currentTime;
    const targetPosition = Math.max(0, currentPosition + seconds);
    if (isTranscoding) {
      state.activePlaybackStartMs = Math.floor(targetPosition * 1000);
      render(false);
      return;
    }
    if (!Number.isFinite(player.duration)) {
      player.currentTime = targetPosition;
      return;
    }
    player.currentTime = Math.min(player.duration, targetPosition);
  };

  const togglePlayback = (): void => {
    if (player.paused) {
      void player.play();
    } else {
      player.pause();
    }
  };

  const toggleFullscreen = (): void => {
    const fullscreenElement = document.fullscreenElement;
    if (fullscreenElement) {
      void document.exitFullscreen();
      return;
    }
    void shell?.requestFullscreen?.();
  };

  shell?.focus({ preventScroll: true });
  ['mousemove', 'mousedown', 'touchstart', 'pointermove'].forEach((eventName) => {
    shell?.addEventListener(eventName, showControls, { passive: true });
  });
  shell?.addEventListener('keydown', (event) => {
    if (event.target instanceof HTMLInputElement) {
      return;
    }
    if (event.key === ' ' || event.key === 'k') {
      event.preventDefault();
      togglePlayback();
    } else if (event.key === 'ArrowLeft') {
      event.preventDefault();
      seekBy(-10);
    } else if (event.key === 'ArrowRight') {
      event.preventDefault();
      seekBy(30);
    } else if (event.key === 'm') {
      event.preventDefault();
      player.muted = !player.muted;
      updateMuteButton();
    } else if (event.key === 'f') {
      event.preventDefault();
      toggleFullscreen();
    } else if (event.key === 'Escape') {
      event.preventDefault();
      closeActivePlaybackSession();
    }
    showControls();
  });

  playButtons.forEach((button) => button.addEventListener('click', () => {
    togglePlayback();
    showControls();
  }));
  document.querySelectorAll<HTMLButtonElement>('[data-player-seek]').forEach((button) => {
    button.addEventListener('click', () => {
      const requestedSeconds = Number(button.dataset.playerSeek);
      const direction = Math.sign(requestedSeconds);
      if (direction !== 0) {
        seekWithEscalation(direction);
      }
      showControls();
    });
  });
  muteButton?.addEventListener('click', () => {
    player.muted = !player.muted;
    updateMuteButton();
    showControls();
  });
  volume?.addEventListener('input', () => {
    player.volume = Number(volume.value);
    player.muted = player.volume === 0;
    updateMuteButton();
    showControls();
  });
  volume?.addEventListener('wheel', (event) => {
    event.preventDefault();
    const delta = event.deltaY < 0 ? 0.05 : -0.05;
    player.volume = Math.min(1, Math.max(0, player.volume + delta));
    player.muted = player.volume === 0;
    updateMuteButton();
    showControls();
  }, { passive: false });
  fullscreenButton?.addEventListener('click', () => {
    toggleFullscreen();
    showControls();
  });
  audioTrackToggle?.addEventListener('click', () => {
    setAudioTrackMenuOpen(!state.isAudioTrackMenuOpen);
    showControls();
  });
  document.querySelectorAll<HTMLButtonElement>('[data-player-audio-track-index]').forEach((button) => {
    button.addEventListener('click', () => {
      const nextAudioTrackIndex = Number(button.dataset.playerAudioTrackIndex);
      if (!Number.isFinite(nextAudioTrackIndex)) {
        return;
      }
      if (nextAudioTrackIndex === currentAudioTrackIndex) {
        setAudioTrackMenuOpen(false);
        showControls();
        return;
      }
      state.activeAudioStreamIndex = nextAudioTrackIndex;
      state.activePlaybackStartMs = Math.floor((playbackBaseOffsetSeconds + player.currentTime) * 1000);
      setAudioTrackMenuOpen(false);
      render(false);
    });
  });
  pipButton?.addEventListener('click', async () => {
    if (!(player instanceof HTMLVideoElement) || !document.pictureInPictureEnabled) {
      state.error = 'Picture in picture is not available in this browser.';
      render();
      return;
    }
    try {
      if (document.fullscreenElement) {
        await document.exitFullscreen();
      }
      if (player.paused) {
        void player.play();
      }
      await player.requestPictureInPicture();
      shell?.classList.add('is-picture-in-picture');
      document.body.style.cursor = '';
    } catch (error) {
      console.error('Failed to enter picture-in-picture', error);
      state.error = error instanceof Error ? error.message : 'Failed to enter picture in picture.';
      render();
    }
  });
  player.addEventListener('leavepictureinpicture', () => {
    shell?.classList.remove('is-picture-in-picture');
    showControls();
  });
  progress?.addEventListener('input', () => {
    isScrubbing = true;
    const duration = sourceDurationSeconds > 0 ? sourceDurationSeconds : Number.isFinite(player.duration) ? player.duration : 0;
    if (duration > 0) {
      const previewSeconds = (Number(progress.value) / 1000) * duration;
      if (currentTimeLabel) {
        currentTimeLabel.textContent = formatMediaTime(previewSeconds);
      }
    }
    showControls();
  });
  progress?.addEventListener('wheel', (event) => {
    event.preventDefault();
    const direction = event.deltaY < 0 ? 1 : -1;
    seekWithEscalation(direction);
    updateTimeline();
    showControls();
  }, { passive: false });
  progress?.addEventListener('change', () => {
    const duration = sourceDurationSeconds > 0 ? sourceDurationSeconds : Number.isFinite(player.duration) ? player.duration : 0;
    if (duration > 0) {
      const targetPosition = (Number(progress.value) / 1000) * duration;
      if (isTranscoding) {
        state.activePlaybackStartMs = Math.floor(targetPosition * 1000);
        render(false);
        return;
      }
      player.currentTime = targetPosition;
    }
    isScrubbing = false;
    updateTimeline();
    showControls();
  });

  let lastSentSeconds = -1;
  player.addEventListener('loadstart', () => setPlayerLoading(true));
  player.addEventListener('waiting', refreshPlayerLoading);
  player.addEventListener('stalled', refreshPlayerLoading);
  player.addEventListener('loadeddata', () => setPlayerLoading(false));
  player.addEventListener('canplay', () => {
    applyInitialDirectSeek();
    setPlayerLoading(false);
  });
  player.addEventListener('playing', () => setPlayerLoading(false));
  player.addEventListener('error', () => {
    setPlayerError();
    console.error('Media playback failed', player.error);
  });
  player.addEventListener('loadedmetadata', () => {
    applyInitialDirectSeek();
    updateTimeline();
  });
  player.addEventListener('play', () => {
    updatePlayButtons();
    showControls();
  });
  player.addEventListener('pause', () => {
    updatePlayButtons();
    showControls();
  });
  player.addEventListener('volumechange', updateMuteButton);
  player.addEventListener('timeupdate', () => {
    setPlayerLoading(false);
    updateTimeline();
    const currentSeconds = Math.floor(player.currentTime);
    if (currentSeconds === lastSentSeconds || currentSeconds % 15 !== 0) {
      return;
    }

    lastSentSeconds = currentSeconds;
    void updatePlaybackProgress(state.selectedItem!.id, {
      position_ms: Math.floor((playbackBaseOffsetSeconds + player.currentTime) * 1000),
      duration_ms: state.selectedItem?.duration_ms ?? (Number.isFinite(player.duration) ? Math.floor(player.duration * 1000) : undefined),
      completed: false,
    });
  });

  player.addEventListener('ended', () => {
    updatePlayButtons();
    showControls();
    void updatePlaybackProgress(state.selectedItem!.id, {
      position_ms: state.selectedItem?.duration_ms ?? Math.floor((playbackBaseOffsetSeconds + (Number.isFinite(player.duration) ? player.duration : 0)) * 1000),
      duration_ms: state.selectedItem?.duration_ms ?? (Number.isFinite(player.duration) ? Math.floor(player.duration * 1000) : undefined),
      completed: true,
    });
  });

  updatePlayButtons();
  updateMuteButton();
  updatePipButton();
  updateTimeline();
  setPlayerLoading(player.readyState < player.HAVE_FUTURE_DATA);
  showControls();
  void player.play().catch((error) => {
    console.warn('Autoplay after opening player was blocked or failed', error);
    updatePlayButtons();
    setPlayerLoading(false);
    showControls();
  });
}

function bindEvents(): void {
  document.querySelector<HTMLFormElement>('#welcome-user-form')?.addEventListener('submit', async (event) => {
    event.preventDefault();
    const form = event.currentTarget as HTMLFormElement | null;
    if (!form) {
      return;
    }

    const formData = new FormData(form);
    const request: CreateUserRequest = {
      username: String(formData.get('username') ?? '').trim(),
      password: String(formData.get('password') ?? ''),
      pin: String(formData.get('pin') ?? '').trim() || undefined,
      admin: true,
      birthday: String(formData.get('birthday') ?? '').trim() || undefined,
      profile_image_url: String(formData.get('profile_image_url') ?? '').trim() || undefined,
      preferred_metadata_languages: parseMetadataLanguageInput(formData.get('preferred_metadata_languages')),
    };

    try {
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
      username: String(formData.get('username') ?? '').trim(),
      password: String(formData.get('password') ?? ''),
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
      navigateTo(`/settings/providers${providerId ? `#provider-${providerId}` : ''}`);
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
    state.searchQuery = input?.value.trim() ?? '';
    state.showFullSearchResults = Boolean(state.searchQuery);
    void refreshData();
  });

  document.querySelector<HTMLInputElement>('#search-input')?.addEventListener('input', (event) => {
    const input = event.currentTarget as HTMLInputElement;
    state.searchQuery = input.value.trim();
    state.showFullSearchResults = false;
    if (pendingLiveSearchHandle !== undefined) {
      window.clearTimeout(pendingLiveSearchHandle);
    }
    pendingLiveSearchHandle = window.setTimeout(() => {
      pendingLiveSearchHandle = undefined;
      void refreshData(false);
    }, 250);
  });

  document.querySelector<HTMLButtonElement>('#refresh-active-library-metadata')?.addEventListener('click', async () => {
    const button = document.querySelector<HTMLButtonElement>('#refresh-active-library-metadata');
    const library = activeLibrary();
    if (!library || libraryRefreshProgress(library)) {
      return;
    }

    try {
      setButtonBusy(button, true);
      const refreshedLibrary = await refreshLibraryMetadata(library.id);
      state.libraries = state.libraries.map((entry) => entry.id === refreshedLibrary.id ? refreshedLibrary : entry);
      await refreshPendingMetadataData();
      schedulePendingMetadataRefresh();
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
        const nextPath = homeBrowsePath();
        window.history.pushState({}, '', nextPath);
        state.route = parseRoute();
        void refreshData();
        return;
      }

      if (state.homeTab === nextTab) {
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
      if (!collection) {
        return;
      }

      navigateTo(browseDetailPath('collection', collection.id));
    });
  });

  document.querySelector<HTMLButtonElement>('#clear-browse-filter')?.addEventListener('click', () => {
    state.browseFilter = undefined;
    navigateTo(typeof activeLibraryId() === 'number' ? `/libraries/${activeLibraryId()}` : '/');
  });

  document.querySelectorAll<HTMLElement>('[data-item-id]').forEach((button) => {
    button.addEventListener('click', () => {
      const itemId = Number(button.dataset.itemId);
      if (!Number.isFinite(itemId)) {
        return;
      }

      navigateTo(`/items/${itemId}`);
    });
  });

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
      window.history.back();
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
        window.clearTimeout(trailerHoldHandle);
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

      trailerHoldHandle = window.setTimeout(() => {
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

      trailerHoldHandle = window.setTimeout(() => {
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
      state.activePlaybackStartMs = Number.isFinite(startMs) ? Math.max(0, startMs) : 0;
      state.isPlayerOpen = true;
      state.activeAudioStreamIndex = undefined;
      state.isAudioTrackMenuOpen = false;
      render();
      state.activePlaybackSession = await createPlaybackSession({
        item_id: state.selectedItem.id,
        client_profile: getWebClientProfile(),
      });
      render();
    } catch (error) {
      state.error = error instanceof Error ? error.message : 'Failed to start playback session.';
      state.isPlayerOpen = false;
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
    const confirmed = window.confirm('Clear cached provider metadata responses? The next metadata refresh will fetch fresh data from providers.');
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

  document.querySelector<HTMLFormElement>('#metadata-dashboard-filter-form')?.addEventListener('submit', (event) => {
    event.preventDefault();
    const form = event.currentTarget as HTMLFormElement | null;
    if (!form) {
      return;
    }

    const formData = new FormData(form);
    state.metadataDashboardFilters = {
      libraryId: String(formData.get('dashboard_library_id') ?? '').trim(),
      itemType: String(formData.get('dashboard_item_type') ?? '').trim(),
      refreshState: String(formData.get('dashboard_refresh_state') ?? '').trim(),
      search: String(formData.get('dashboard_search') ?? '').trim(),
    };
    const root = document.querySelector<HTMLElement>('#metadata-dashboard-panel-root');
    if (!root) {
      render();
      return;
    }
    root.innerHTML = renderMetadataDashboard();
    createIcons({ icons });
    bindEvents();
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
    root.innerHTML = renderMetadataDashboard();
    createIcons({ icons });
    bindEvents();
  });

  document.querySelector<HTMLFormElement>('#log-filter-form')?.addEventListener('submit', async (event) => {
    event.preventDefault();
    const form = event.currentTarget as HTMLFormElement | null;
    if (!form) {
      return;
    }

    const formData = new FormData(form);
    state.logFilters = {
      level: String(formData.get('log_level') ?? '').trim().toUpperCase(),
      module: String(formData.get('log_module') ?? '').trim(),
      search: String(formData.get('log_search') ?? '').trim(),
      since: String(formData.get('log_since') ?? '').trim(),
      until: String(formData.get('log_until') ?? '').trim(),
    };
    await refreshLogsView();
  });

  document.querySelector<HTMLButtonElement>('#clear-log-filters')?.addEventListener('click', async () => {
    state.logFilters = {
      level: '',
      module: '',
      search: '',
      since: '',
      until: '',
    };
    await refreshLogsView();
  });

  document.querySelector<HTMLButtonElement>('#refresh-log-viewer')?.addEventListener('click', async () => {
    await refreshLogsView();
  });

  document.querySelectorAll<HTMLButtonElement>('[data-shelf-scroll]').forEach((button) => {
    button.addEventListener('click', () => {
      const [shelfId, directionValue] = (button.dataset.shelfScroll ?? '').split(':');
      const direction = Number(directionValue);
      const row = document.querySelector<HTMLElement>(`[data-shelf-row="${CSS.escape(shelfId)}"]`);
      if (!row || !Number.isFinite(direction)) {
        return;
      }
      row.scrollBy({ left: direction * Math.max(320, row.clientWidth * 0.8), behavior: 'smooth' });
    });
  });

  const bindHomeFeatureAction = (): void => {
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
  };

  const updatePageBackdrop = (backdropUrl: string | undefined): void => {
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
  };

  document.querySelectorAll<HTMLElement>('[data-preview-item-id]').forEach((element) => {
    const updatePreview = (): void => {
      if (state.route.page === 'browse-detail') {
        return;
      }

      const itemId = Number(element.dataset.previewItemId);
      if (!Number.isFinite(itemId) || state.homePreviewItemId === itemId) {
        return;
      }
      state.homePreviewItemId = itemId;
      state.homePreviewCollectionId = undefined;
      const highlightedItem = homePreviewCandidates().find((item) => item.id === itemId);
      const previewItem = highlightedItem ? showPreviewItemForHighlight(highlightedItem) : undefined;
      const root = document.querySelector<HTMLElement>('.home-feature');
      if (root) {
        root.outerHTML = renderHomeFeature();
        createIcons({ icons });
        bindHomeFeatureAction();
      }
      updatePageBackdrop(pageBackdropUrlForItem(previewItem));
    };
    element.addEventListener('mouseenter', updatePreview);
    element.addEventListener('focus', updatePreview);
  });

  document.querySelectorAll<HTMLElement>('[data-preview-collection-id]').forEach((element) => {
    const updatePreview = (): void => {
      const collectionId = element.dataset.previewCollectionId;
      if (!collectionId || state.homePreviewCollectionId === collectionId) {
        return;
      }
      state.homePreviewCollectionId = collectionId;
      state.homePreviewItemId = undefined;
      const collection = collectionSummaries().find((entry) => entry.id === collectionId);
      const root = document.querySelector<HTMLElement>('.home-feature');
      if (root) {
        root.outerHTML = renderHomeFeature();
        createIcons({ icons });
        bindHomeFeatureAction();
      }
      updatePageBackdrop(pageBackdropUrlForCollection(collection));
    };
    element.addEventListener('mouseenter', updatePreview);
    element.addEventListener('focus', updatePreview);
  });
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

      const formData = new FormData(form);
      const request: UpdateUserRequest = {
        username: String(formData.get('username') ?? '').trim(),
        admin: formData.get('admin') === 'on',
        birthday: String(formData.get('birthday') ?? '').trim() || undefined,
        profile_image_url: String(formData.get('profile_image_url') ?? '').trim() || undefined,
        preferred_metadata_languages: parseMetadataLanguageInput(formData.get('preferred_metadata_languages')),
      };

      try {
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

    const formData = new FormData(form);
    const request: CreateUserRequest = {
      username: String(formData.get('username') ?? '').trim(),
      password: String(formData.get('password') ?? ''),
      pin: String(formData.get('pin') ?? '').trim() || undefined,
      admin: formData.get('admin') === 'on',
      birthday: String(formData.get('birthday') ?? '').trim() || undefined,
      profile_image_url: String(formData.get('profile_image_url') ?? '').trim() || undefined,
      preferred_metadata_languages: parseMetadataLanguageInput(formData.get('preferred_metadata_languages')),
    };

    try {
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

      const confirmed = window.confirm('Remove this library from settings? This only removes the configuration, not the media files on disk.');
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
        schedulePendingMetadataRefresh();
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
      name: String(formData.get('library_name') ?? ''),
      path: paths[0] ?? '',
      paths,
      recursive: formData.get('library_recursive') === 'on',
      kind: String(formData.get('library_kind') ?? 'movies'),
      metadata_providers: formData.getAll('library_metadata_provider').map((value) => String(value)),
      metadata_language_mode: String(formData.get('library_metadata_language_mode') ?? 'auto') === 'manual' ? 'manual' : 'auto',
      metadata_languages: normalizedMetadataLanguages(formData.getAll('library_metadata_language').map((value) => String(value))),
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

window.addEventListener('keydown', (event) => {
  if (state.isPlayerOpen || state.activeTrailer || event.defaultPrevented) {
    return;
  }
  if (!['ArrowRight', 'ArrowLeft', 'ArrowDown', 'ArrowUp'].includes(event.key)) {
    return;
  }
  const target = event.target as HTMLElement | null;
  if (target?.matches('input, textarea, select, [contenteditable="true"]')) {
    return;
  }
  const focusable = Array.from(
    document.querySelectorAll<HTMLElement>(
      'button:not(:disabled), a[href], input:not(:disabled), select:not(:disabled), textarea:not(:disabled), [tabindex]:not([tabindex="-1"])',
    ),
  ).filter((element) => element.offsetParent !== null);
  if (!focusable.length) {
    return;
  }
  const currentIndex = Math.max(0, focusable.indexOf(document.activeElement as HTMLElement));
  const direction = event.key === 'ArrowRight' || event.key === 'ArrowDown' ? 1 : -1;
  focusable[(currentIndex + direction + focusable.length) % focusable.length]?.focus();
  event.preventDefault();
});

function moveFocus(direction: 1 | -1): void {
  const focusable = Array.from(
    document.querySelectorAll<HTMLElement>(
      'button:not(:disabled), a[href], input:not(:disabled), select:not(:disabled), textarea:not(:disabled), [tabindex]:not([tabindex="-1"])',
    ),
  ).filter((element) => element.offsetParent !== null);
  if (!focusable.length) {
    return;
  }
  const currentIndex = Math.max(0, focusable.indexOf(document.activeElement as HTMLElement));
  focusable[(currentIndex + direction + focusable.length) % focusable.length]?.focus();
}

function activateFocusedElement(): void {
  const focused = document.activeElement as HTMLElement | null;
  if (focused?.matches('button, a[href], input, select, textarea')) {
    focused.click();
  }
}

function pollGamepads(): void {
  const gamepads = navigator.getGamepads?.() ?? [];
  gamepads.forEach((gamepad) => {
    if (!gamepad) {
      return;
    }
    const actions: Array<[string, boolean, () => void]> = [
      ['up', Boolean(gamepad.buttons[12]?.pressed) || gamepad.axes[1] < -0.65, () => moveFocus(-1)],
      ['down', Boolean(gamepad.buttons[13]?.pressed) || gamepad.axes[1] > 0.65, () => moveFocus(1)],
      ['left', Boolean(gamepad.buttons[14]?.pressed) || gamepad.axes[0] < -0.65, () => moveFocus(-1)],
      ['right', Boolean(gamepad.buttons[15]?.pressed) || gamepad.axes[0] > 0.65, () => moveFocus(1)],
      ['activate', Boolean(gamepad.buttons[0]?.pressed), activateFocusedElement],
      ['back', Boolean(gamepad.buttons[1]?.pressed), () => window.history.back()],
    ];
    actions.forEach(([name, pressed, action]) => {
      const key = `${gamepad.index}:${name}`;
      if (pressed && !activeGamepadButtons.has(key)) {
        activeGamepadButtons.add(key);
        action();
      } else if (!pressed) {
        activeGamepadButtons.delete(key);
      }
    });
  });
  window.requestAnimationFrame(pollGamepads);
}

window.requestAnimationFrame(pollGamepads);

window.addEventListener('popstate', () => {
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

