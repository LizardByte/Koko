import './style.css';
import kokoLogoUrl from '../../../assets/Koko.svg';
import { createIcons, icons } from 'lucide';
import {
  addLibrary,
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
  getMetadataProviders,
  getLogs,
  getPlaybackDecision,
  getSystemActivities,
  refreshLibraryMetadata,
  refreshItemMetadata,
  getSettings,
  getStoredAuthToken,
  getStoredApiBase,
  getStreamUrl,
  getUsers,
  linkItemMetadata,
  loginUser,
  resolveApiUrl,
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
  type ItemMetadataResponse,
  type LoginRequest,
  type MediaHome,
  type MediaItemDetail,
  type MediaItemSummary,
  type MediaLibrary,
  type MediaLibrarySettings,
  type MetadataProviderStatus,
  type MetadataSearchResult,
  type LogEntriesResponse,
  type PlaybackDecision,
  type SettingsResponse,
  type SettingsSnapshot,
  type ServerCapabilities,
  type SystemActivity,
} from './api';

type AppRoute =
  | { page: 'home'; libraryId?: number }
  | { page: 'item'; itemId: number }
  | { page: 'settings'; section?: SettingsSection };

type HomeBrowseTab = 'recommended' | 'library' | 'collections' | 'playlists' | 'categories';
type SettingsSection = 'general' | 'libraries' | 'dashboard' | 'logs';

interface BrowseFilter {
  kind: 'category' | 'collection';
  label: string;
  itemIds: number[];
}

interface TrailerOption {
  title: string;
  url: string;
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
  searchResults: MediaItemSummary[];
  metadataProviders: MetadataProviderStatus[];
  systemActivities: SystemActivity[];
  dashboardItems: MediaItemSummary[];
  settingsResponse?: SettingsResponse;
  logsResponse?: LogEntriesResponse;
  selectedItem?: MediaItemDetail;
  selectedItemMetadata?: ItemMetadataResponse;
  selectedPlayback?: PlaybackDecision;
  metadataSearchResults: MetadataSearchResult[];
  searchQuery: string;
  metadataSearchQuery: string;
  homeTab: HomeBrowseTab;
  browseFilter?: BrowseFilter;
  isLoading: boolean;
  isPlayerOpen: boolean;
  isTrailerMenuOpen: boolean;
  activeTrailer?: { title: string; url: string };
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
  | 'book'
  | 'clapperboard'
  | 'film'
  | 'house'
  | 'image'
  | 'layout-grid'
  | 'link-2'
  | 'log-in'
  | 'log-out'
  | 'music'
  | 'play'
  | 'plus'
  | 'refresh-cw'
  | 'save'
  | 'search'
  | 'settings'
  | 'trash-2'
  | 'tv'
  | 'triangle-alert'
  | 'user-plus'
  | 'x';

const state: AppState = {
  apiBase: getStoredApiBase(),
  apiMode: getApiMode(),
  route: parseRoute(),
  users: [],
  libraries: [],
  libraryItems: [],
  searchResults: [],
  metadataProviders: [],
  systemActivities: [],
  dashboardItems: [],
  metadataSearchResults: [],
  searchQuery: '',
  metadataSearchQuery: '',
  homeTab: defaultHomeTab(parseRoute()),
  isLoading: true,
  hasDeferredAutoRefreshRender: false,
  isPlayerOpen: false,
  isTrailerMenuOpen: false,
  activeTrailer: undefined,
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
let pendingLibraryRefreshHandle: number | undefined;
let pendingMetadataRefreshHandle: number | undefined;

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

  const settingsMatch = normalizedPath.match(/^\/settings(?:\/(libraries|dashboard|logs))?$/);
  if (settingsMatch) {
    return { page: 'settings', section: (settingsMatch[1] as SettingsSection | undefined) ?? 'general' };
  }

  const itemMatch = normalizedPath.match(/^\/items\/(\d+)$/);
  if (itemMatch) {
    return { page: 'item', itemId: Number(itemMatch[1]) };
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
  return state.route.page !== 'item'
    && state.libraries.some((library) => library.status === 'never_scanned');
}

function schedulePendingLibraryRefresh(): void {
  clearPendingLibraryRefresh();
  if (!shouldAutoRefreshLibraries()) {
    return;
  }

  pendingLibraryRefreshHandle = window.setTimeout(() => {
    pendingLibraryRefreshHandle = undefined;
    void refreshData();
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

function extractYouTubeVideoId(url: string): string | undefined {
  const normalizedUrl = url.trim();
  if (!normalizedUrl) {
    return undefined;
  }

  if (/^[A-Za-z0-9_-]{11}$/.test(normalizedUrl)) {
    return normalizedUrl;
  }

  try {
    const parsed = new URL(normalizedUrl);
    const host = parsed.hostname.toLowerCase();
    if (host === 'youtu.be') {
      const videoId = parsed.pathname.split('/').filter(Boolean)[0];
      return /^[A-Za-z0-9_-]{11}$/.test(videoId ?? '') ? videoId : undefined;
    }

    if (host.endsWith('youtube.com')) {
      if (parsed.pathname.startsWith('/watch')) {
        const videoId = parsed.searchParams.get('v')?.trim();
        return /^[A-Za-z0-9_-]{11}$/.test(videoId ?? '') ? videoId : undefined;
      }

      if (parsed.pathname.startsWith('/embed/')) {
        const videoId = parsed.pathname.split('/')[2]?.trim();
        return /^[A-Za-z0-9_-]{11}$/.test(videoId ?? '') ? videoId : undefined;
      }
    }
  } catch {
    return undefined;
  }

  return undefined;
}

function buildYouTubeEmbedUrl(
  url: string,
  options: { autoplay?: boolean; controls?: boolean; loop?: boolean } = {},
): string | undefined {
  const videoId = extractYouTubeVideoId(url);
  if (!videoId) {
    return undefined;
  }

  const embedUrl = new URL(`https://www.youtube.com/embed/${videoId}`);
  embedUrl.searchParams.set('rel', '0');
  embedUrl.searchParams.set('playsinline', '1');
  embedUrl.searchParams.set('modestbranding', '1');
  embedUrl.searchParams.set('enablejsapi', '1');
  if (window.location.origin) {
    embedUrl.searchParams.set('origin', window.location.origin);
  }
  embedUrl.searchParams.set('autoplay', options.autoplay ? '1' : '0');
  embedUrl.searchParams.set('controls', options.controls === false ? '0' : '1');
  if (options.loop) {
    embedUrl.searchParams.set('loop', '1');
    embedUrl.searchParams.set('playlist', videoId);
  }

  return embedUrl.toString();
}

function parseTrailerOptionsFromPayload(payloadJson?: string): TrailerOption[] {
  if (!payloadJson) {
    return [];
  }

  try {
    const payload = JSON.parse(payloadJson) as { videos?: { results?: Array<Record<string, unknown>> } };
    const results = Array.isArray(payload.videos?.results) ? payload.videos.results : [];
    const seenVideoIds = new Set<string>();

    return results
      .filter((entry) => entry.site === 'YouTube' && (entry.type === 'Trailer' || entry.type === 'Teaser'))
      .sort((left, right) => {
        const score = (entry: Record<string, unknown>): number => {
          const type = entry.type === 'Trailer' ? 'Trailer' : entry.type === 'Teaser' ? 'Teaser' : '';
          const official = entry.official === true;
          if (official && type === 'Trailer') {
            return 0;
          }

          if (official && type === 'Teaser') {
            return 1;
          }

          if (type === 'Trailer') {
            return 2;
          }

          if (type === 'Teaser') {
            return 3;
          }

          return 4;
        };

        return score(left) - score(right);
      })
      .flatMap((entry, index) => {
        const videoId = typeof entry.key === 'string' ? entry.key.trim() : '';
        if (!videoId || seenVideoIds.has(videoId)) {
          return [];
        }

        seenVideoIds.add(videoId);
        return [{
          title: typeof entry.name === 'string' && entry.name.trim()
            ? entry.name.trim()
            : `Trailer ${index + 1}`,
          url: `https://www.youtube.com/watch?v=${videoId}`,
        } satisfies TrailerOption];
      });
  } catch {
    return [];
  }
}

function currentTrailerOptions(): TrailerOption[] {
  const parsedOptions = parseTrailerOptionsFromPayload(state.selectedItemMetadata?.matches[0]?.provider_payload_json);
  if (parsedOptions.length) {
    return parsedOptions;
  }

  if (!state.selectedItem?.trailer_url) {
    return [];
  }

  return [{
    title: state.selectedItem.trailer_title?.trim() || 'Trailer',
    url: state.selectedItem.trailer_url,
  }];
}

function openTrailer(option: TrailerOption | undefined): void {
  if (!option) {
    return;
  }

  state.activeTrailer = option;
  state.isTrailerMenuOpen = false;
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
  if (state.route.page === 'home') {
    return state.route.libraryId;
  }

  return state.selectedItem?.library_id;
}

function activeLibrary(): MediaLibrary | undefined {
  return state.libraries.find((library) => library.id === activeLibraryId());
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

function selectedLibraryName(): string {
  if (state.route.page === 'settings') {
    return 'Settings';
  }

  return activeLibrary()?.name ?? (state.route.page === 'item' ? 'Item details' : 'Home');
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
      return 'Scanning';
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

function filteredTopLevelLibraryItems(): MediaItemSummary[] {
  const items = topLevelLibraryItems();
  if (!state.browseFilter) {
    return items;
  }

  const allowedIds = new Set(state.browseFilter.itemIds);
  return items.filter((item) => allowedIds.has(item.id));
}

function applyBrowseFilter(filter: BrowseFilter): void {
  state.browseFilter = filter;
  state.homeTab = 'library';
  render();
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
    <button class="media-card ${item.item_type === 'episode' ? 'episode-card' : ''}" type="button" data-item-id="${item.id}">
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

function renderShelfStack(): string {
  if (state.searchQuery.trim()) {
    if (!state.searchResults.length) {
      return '<section class="shelf"><div class="empty-state">No media items matched the current search.</div></section>';
    }

    return `
      <section class="shelf">
        <div class="shelf-header">
          <h3>Search results</h3>
          <span>${state.searchResults.length} matches</span>
        </div>
        <div class="item-grid">${state.searchResults.map(renderItemCard).join('')}</div>
      </section>
    `;
  }

  const shelves = state.home?.shelves ?? [];
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
        ${shelf.items.length
          ? `<div class="shelf-row">${shelf.items.map(renderItemCard).join('')}</div>`
          : '<div class="empty-state shelf-empty">Nothing here yet.</div>'}
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
    <nav class="browse-tabs panel page-panel" aria-label="Browse views">
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

function renderLibraryOverview(): string {
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
            <strong>${state.libraries.some((entry) => entry.status === 'never_scanned') ? 'Scanning' : 'Ready'}</strong>
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
      ${library.status === 'never_scanned' ? '<p class="muted library-overview-note">Koko is scanning this library in the background. New items will appear automatically.</p>' : ''}
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
    if (state.browseFilter) {
      return `<div class="empty-state">No items matched the current ${escapeHtml(state.browseFilter.kind)} filter.</div>`;
    }

    if (library?.status === 'never_scanned') {
      return '<div class="empty-state">Koko is scanning this library right now. The show, season, and episode hierarchy will appear when the scan completes.</div>';
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
    <section class="category-grid">
      ${collections.map((collection) => `
        <button
          type="button"
          class="category-card panel filter-card-button"
          data-collection-filter="${escapeHtml(collection.id)}"
          style="${collection.backdrop_url || collection.artwork_url ? `--collection-card-image: url('${escapeHtml(collection.backdrop_url ?? collection.artwork_url ?? '')}');` : ''}"
        >
          <div class="category-card-header">
            <strong>${escapeHtml(collection.name)}</strong>
            <span class="tag">${collection.item_count} title${collection.item_count === 1 ? '' : 's'}</span>
          </div>
          <p class="muted">${escapeHtml(collection.overview ?? 'Open this collection to filter the library view.')}</p>
        </button>
      `).join('')}
    </section>
  `;
}

function renderPlaylistsTab(): string {
  return `
    <section class="placeholder-stack">
      <div class="empty-state">Playlist creation is planned. This tab will eventually let you build reusable watch queues and listening sessions.</div>
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
  if (state.searchQuery.trim()) {
    return renderShelfStack();
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
  const activeLibraryName = selectedLibraryName();
  const library = activeLibrary();
  const activeLibraryPaths = library?.paths ?? [];
  const libraryRefreshPending = library ? Boolean(libraryRefreshProgress(library)) : false;

  return `
    ${renderPageNavbar(
      'Browse',
      activeLibraryName,
      activeLibraryPaths.length ? `${activeLibraryPaths.length} folder${activeLibraryPaths.length === 1 ? '' : 's'} connected for this library.` : 'Browse every configured library from one place.',
      `
        <div class="content-navbar-actions-stack">
          <form id="search-form" class="search-form">
            <input id="search-input" name="search" type="search" value="${escapeHtml(state.searchQuery)}" placeholder="Search titles or relative paths" />
            <button type="submit">${renderButtonContent('Search', 'search')}</button>
            <button id="reset-search" class="secondary-button" type="button">${renderButtonContent('Reset', 'x')}</button>
          </form>
          ${library
            ? `<button type="button" class="secondary-button" id="refresh-active-library-metadata" ${libraryRefreshPending ? 'disabled' : ''}>${renderButtonContent(libraryRefreshPending ? 'Refreshing library metadata' : 'Refresh library metadata', 'refresh-cw')}</button>`
            : ''}
        </div>
      `,
    )}
    ${renderHomeTabs()}
    ${renderLibraryOverview()}
    <section class="shelf-stack panel page-panel">${renderHomeTabContent()}</section>
  `;
}

function renderMetadataSearchResults(): string {
  const selectedItem = state.selectedItem;
  if (!selectedItem) {
    return '';
  }

  if (!state.metadataSearchResults.length) {
    return '<div class="empty-state tight">Run a TMDB search to link rich metadata and artwork.</div>';
  }

  return state.metadataSearchResults
    .map((result) => `
      <article class="metadata-search-card">
        <div>
          <strong>${escapeHtml(result.title)}</strong>
          <p>${escapeHtml(result.overview ?? 'No overview available.')}</p>
          <div class="metadata-match-meta">
            <span>${result.release_year ?? 'Unknown year'}</span>
            <span>${escapeHtml(result.media_type)}</span>
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

function renderLinkedMetadataSummary(): string {
  const linkedMatch = state.selectedItemMetadata?.matches[0];
  if (!linkedMatch) {
    return '<div class="empty-state tight">No external metadata is linked yet.</div>';
  }

  const metadataRefreshPending = itemIsMetadataPending(state.selectedItem);
  const refreshStateLabel = metadataRefreshPending || linkedMatch.refresh_state === 'pending'
    ? 'Refreshing'
    : linkedMatch.refresh_state === 'error'
      ? 'Refresh failed'
      : 'Up to date';

  return `
    <div class="metadata-current-link">
      <span class="tag success">${escapeHtml(linkedMatch.provider_id)}</span>
      <span class="tag">${escapeHtml(linkedMatch.media_type ?? 'linked')}</span>
      <span class="tag ${metadataRefreshPending || linkedMatch.refresh_state === 'pending' ? 'warning' : linkedMatch.refresh_state === 'error' ? 'danger-tag' : ''}">${escapeHtml(refreshStateLabel)}</span>
      ${linkedMatch.release_year ? `<span class="tag">${linkedMatch.release_year}</span>` : ''}
      <span class="metadata-current-copy">
        <strong>${escapeHtml(linkedMatch.title ?? linkedMatch.external_id)}</strong>
        <span class="muted">Last refreshed ${escapeHtml(formatTimestamp(linkedMatch.last_refreshed_at ?? linkedMatch.updated_at))}</span>
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
          <h2>${escapeHtml(state.selectedItem.display_title)}</h2>
          ${state.selectedItem.tagline ? `<p class="hero-tagline">${escapeHtml(state.selectedItem.tagline)}</p>` : ''}
          <div class="hero-meta-row">
            ${state.selectedItem.release_year ? `<span class="tag">${state.selectedItem.release_year}</span>` : ''}
            ${genres.map((genre) => `<span class="tag">${escapeHtml(genre)}</span>`).join('')}
          </div>
          <p class="hero-description">${escapeHtml(overview)}</p>
          <div class="detail-actions">
            ${state.selectedItem.playable ? `<button type="button" id="play-selected-item" ${playback?.can_direct_play ? '' : 'disabled'}>${renderButtonContent(playback?.can_direct_play ? 'Play now' : 'Transcode planned', 'play')}</button>` : ''}
            ${preferredTrailer ? `<button type="button" class="secondary-button" id="play-item-trailer" title="${escapeHtml(trailerButtonTitle)}">${renderButtonContent('Play Trailer', 'play')}</button>` : ''}
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

      ${children.length ? `
        <section class="panel page-panel item-section">
          <div class="section-heading section-heading-actions">
            <h3>${escapeHtml(childSectionTitle)}</h3>
            <span class="muted">${children.length} item${children.length === 1 ? '' : 's'}</span>
          </div>
          <div class="item-grid hierarchy-item-grid ${state.selectedItem.item_type === 'season' ? 'season-episodes-grid' : ''}">${children.map(renderItemCard).join('')}</div>
        </section>
      ` : ''}

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
                <input id="metadata-search-input" name="metadataSearch" type="search" value="${escapeHtml(state.metadataSearchQuery)}" placeholder="Search TMDB or leave blank to use the item title" />
                <button type="submit">${renderButtonContent('Search TMDB', 'search')}</button>
              </form>
              <div class="metadata-search-list">${renderMetadataSearchResults()}</div>
            `
            : '<div class="empty-state tight">Season and episode metadata is inherited and refreshed automatically from the linked show.</div>'}
        </section>
      </section>
    </section>
  `;
}

const metadataProviderKinds: Record<string, string[]> = {
  tmdb: ['movies', 'shows'],
  musicbrainz: ['music'],
  open_library: ['books'],
  local_nfo: ['movies', 'shows', 'music', 'photos', 'books', 'home_videos'],
};

function metadataProviderCheckboxes(prefix: string, selectedProviders: string[], libraryKind?: string): string {
  return Object.keys(metadataProviderKinds)
    .filter((providerId) => !libraryKind || metadataProviderKinds[providerId].includes(libraryKind))
    .map((providerId) => `
      <label class="checkbox-inline">
        <input
          name="${prefix}"
          type="checkbox"
          value="${providerId}"
          data-provider-kinds="${metadataProviderKinds[providerId].join(',')}"
          ${selectedProviders.includes(providerId) ? 'checked' : ''}
        />
        ${providerId}
      </label>
    `)
    .join('');
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
              ? `<button type="button" class="secondary-button" data-refresh-library-id="${persistedLibrary.id}" ${refreshPending ? 'disabled' : ''}>${renderButtonContent(refreshLabel, 'refresh-cw')}</button>`
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
        <fieldset>
          <legend>Metadata sources</legend>
          <div class="checkbox-row">${metadataProviderCheckboxes(`existing_library_metadata_provider_${index}`, library.metadata_providers, library.kind)}</div>
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

function renderSettingsPage(): string {
  const settings = state.settingsResponse?.settings;
  if (!settings) {
    return '<section class="panel page-panel"><div class="empty-state">Settings are still loading…</div></section>';
  }

  const tmdb = settings.metadata.providers.find((provider) => provider.id === 'tmdb');
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
          <div class="form-row checkbox-row">
            <label><input name="tmdb_enabled" type="checkbox" ${tmdb?.enabled ? 'checked' : ''} /> Enable TMDB</label>
          </div>
          <div class="form-row">
            <label>TMDB API key<input name="tmdb_api_key" value="${escapeHtml(tmdb?.api_key ?? '')}" /></label>
            <label>TMDB language<input name="tmdb_language" value="${escapeHtml(tmdb?.language ?? 'en-US')}" /></label>
          </div>
          <div class="form-row">
            <label>TMDB rate limit (requests/second)<input name="tmdb_rate_limit_per_second" type="number" min="1" value="${tmdb?.rate_limit_per_second ?? 4}" /></label>
            <label>TMDB retry attempts<input name="tmdb_retry_attempts" type="number" min="0" value="${tmdb?.retry_attempts ?? 3}" /></label>
          </div>
          <div class="form-row">
            <label>TMDB retry backoff (ms)<input name="tmdb_retry_backoff_ms" type="number" min="0" step="100" value="${tmdb?.retry_backoff_ms ?? 1000}" /></label>
            <label>Automatic refresh
              <select name="metadata_refresh_interval_days">
                <option value="30" ${settings.metadata.refresh_interval_days === 30 ? 'selected' : ''}>Every 30 days</option>
                <option value="60" ${settings.metadata.refresh_interval_days === 60 ? 'selected' : ''}>Every 60 days</option>
                <option value="90" ${settings.metadata.refresh_interval_days === 90 ? 'selected' : ''}>Every 90 days</option>
                <option value="never" ${settings.metadata.refresh_interval_days == null ? 'selected' : ''}>Never</option>
              </select>
            </label>
          </div>
        </section>

        <div class="page-actions">
          <button type="submit">${renderButtonContent('Save settings', 'save')}</button>
          <button type="button" class="secondary-button" id="go-home-from-settings">${renderButtonContent('Back home', 'house')}</button>
        </div>
      </form>

      ${renderUserManagement()}
    </section>
    ` : ''}
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
            <fieldset>
              <legend>Metadata sources</legend>
              <div class="checkbox-row" id="add-library-metadata-providers">${metadataProviderCheckboxes('library_metadata_provider', ['tmdb'])}</div>
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
    case 'settings':
      return renderSettingsPage();
    default:
      return renderHomePage();
  }
}

function renderPlayerOverlay(): string {
  if (state.activeTrailer) {
    const trailerUrl = buildYouTubeEmbedUrl(state.activeTrailer.url, { autoplay: true, controls: true })
      ?? state.activeTrailer.url;
    return `
      <div class="player-overlay trailer-overlay">
        <div class="player-shell trailer-shell">
          <div class="player-header">
            <div>
              <p class="eyebrow">Trailer</p>
              <h2>${escapeHtml(state.activeTrailer.title)}</h2>
            </div>
            <button id="close-trailer" class="secondary-button" type="button">${renderButtonContent('Close', 'x')}</button>
          </div>
          <div class="trailer-frame-shell">
            <iframe
              id="trailer-player"
              src="${escapeHtml(trailerUrl)}"
              title="${escapeHtml(state.activeTrailer.title)} trailer"
              allow="accelerometer; autoplay; encrypted-media; gyroscope; picture-in-picture; fullscreen"
              referrerpolicy="origin"
              allowfullscreen
            ></iframe>
          </div>
        </div>
      </div>
    `;
  }

  if (!state.isPlayerOpen || !state.selectedItem || !state.selectedPlayback?.can_direct_play) {
    return '';
  }

  const tag = state.selectedItem.media_kind === 'audio' ? 'audio' : 'video';
  const source = getStreamUrl(state.selectedItem.id);
  const trackMarkup = tag === 'video'
    ? state.selectedItem.subtitle_tracks
        .map((track) => `<track kind="subtitles" label="${escapeHtml(track.label)}" srclang="${escapeHtml(subtitleLanguage(track.label))}" src="${escapeHtml(resolveApiUrl(track.url))}" />`)
        .join('')
    : '';

  return `
    <div class="player-overlay">
      <div class="player-shell">
        <div class="player-header">
          <div>
            <p class="eyebrow">Now playing</p>
            <h2>${escapeHtml(state.selectedItem.display_title)}</h2>
          </div>
          <button id="close-player" class="secondary-button" type="button">${renderButtonContent('Close', 'x')}</button>
        </div>
        ${tag === 'audio'
          ? `<audio id="media-player" controls autoplay src="${escapeHtml(source)}"></audio>`
          : `<video id="media-player" controls autoplay playsinline src="${escapeHtml(source)}">${trackMarkup}</video>`}
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
  const previousScrollTop = preserveScroll
    ? document.querySelector<HTMLElement>('.main-shell')?.scrollTop ?? 0
    : 0;

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

  const pageBackdropUrl = state.route.page === 'item' && state.selectedItem
    && (state.selectedItem.backdrop_url || state.selectedItemMetadata?.matches.some((match) => Boolean(match.backdrop_url || match.cached_backdrop_path)))
    ? getArtworkUrl(state.selectedItem.id, 'backdrop', state.selectedItem.artwork_updated_at)
    : undefined;
  const railCollapsed = isRailCollapsed();

  appRoot.innerHTML = `
    <div class="app-shell${pageBackdropUrl ? ' has-page-backdrop' : ''}${railCollapsed ? ' rail-collapsed' : ''}">
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
      state.metadataProviders = [];
      state.systemActivities = [];
      state.dashboardItems = [];
      state.settingsResponse = undefined;
      state.logsResponse = undefined;
      state.selectedItem = undefined;
      state.selectedItemMetadata = undefined;
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

    if (state.route.page === 'home') {
      const [home, libraryItems, searchResults] = await Promise.all([
        getHome(state.route.libraryId),
        getItems(state.route.libraryId),
        state.searchQuery.trim()
          ? searchItems(state.searchQuery, state.route.libraryId)
          : Promise.resolve([]),
      ]);
      state.home = home;
      state.libraryItems = libraryItems;
      state.searchResults = searchResults;
      state.selectedItem = undefined;
      state.selectedItemMetadata = undefined;
      state.selectedPlayback = undefined;
      state.metadataSearchResults = [];
      state.metadataSearchQuery = '';
      state.isPlayerOpen = false;
      state.isTrailerMenuOpen = false;
      state.activeTrailer = undefined;
      state.hasDeferredAutoRefreshRender = false;
      state.dashboardItems = [];
      state.logsResponse = undefined;
    } else if (state.route.page === 'item') {
      state.home = undefined;
      state.libraryItems = [];
      state.searchResults = [];
      state.metadataSearchResults = [];
      state.metadataSearchQuery = '';
      state.isTrailerMenuOpen = false;
      state.activeTrailer = undefined;
      state.dashboardItems = [];
      state.logsResponse = undefined;
      const [item, metadata, playback] = await Promise.all([
        getItem(state.route.itemId),
        getItemMetadata(state.route.itemId),
        getPlaybackDecision(state.route.itemId),
      ]);
      state.selectedItem = item;
      state.selectedItemMetadata = metadata;
      state.selectedPlayback = playback;
    } else {
      state.home = undefined;
      state.libraryItems = [];
      state.searchResults = [];
      state.selectedItem = undefined;
      state.selectedItemMetadata = undefined;
      state.selectedPlayback = undefined;
      state.metadataSearchResults = [];
      state.metadataSearchQuery = '';
      state.isPlayerOpen = false;
      state.isTrailerMenuOpen = false;
      state.activeTrailer = undefined;
      state.hasDeferredAutoRefreshRender = false;
      const [logsResponse, dashboardItems] = await Promise.all([
        getLogs(currentLogFilterRequest()),
        getItems(),
      ]);
      state.logsResponse = logsResponse;
      state.dashboardItems = dashboardItems;
    }

    state.apiMode = getApiMode();
  } catch (error) {
    state.error = error instanceof Error ? error.message : 'Failed to load server data.';
    state.apiMode = getApiMode();
  } finally {
    state.isLoading = false;
    schedulePendingLibraryRefresh();
    schedulePendingMetadataRefresh();
    render(false);
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

      state.systemActivities = activitiesResponse.activities;
      state.libraries = libraries;
      state.selectedItem = item;
      state.selectedItemMetadata = metadata;
      state.error = undefined;
      shouldRender = previousSnapshot !== snapshotJson({
        systemActivities: state.systemActivities,
        libraries: state.libraries,
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
          metadata_providers: formData.has(providerField)
            ? formData.getAll(providerField).map((value) => String(value))
            : library.metadata_providers,
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
        if (provider.id !== 'tmdb') {
          return provider;
        }

        if (!formData.has('tmdb_api_key') && !formData.has('tmdb_enabled')) {
          return provider;
        }

        return {
          ...provider,
          enabled: settingsSection === 'general' ? formData.get('tmdb_enabled') === 'on' : provider.enabled,
          api_key: String(formData.get('tmdb_api_key') ?? provider.api_key ?? ''),
          language: String(formData.get('tmdb_language') ?? provider.language),
          rate_limit_per_second: Math.max(1, Number(formData.get('tmdb_rate_limit_per_second') ?? provider.rate_limit_per_second)),
          retry_attempts: Math.max(0, Number(formData.get('tmdb_retry_attempts') ?? provider.retry_attempts)),
          retry_backoff_ms: Math.max(1, Number(formData.get('tmdb_retry_backoff_ms') ?? provider.retry_backoff_ms)),
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

function currentThemeSongSource(): { kind: 'audio' | 'youtube'; src: string; title: string } | undefined {
  if (state.route.page !== 'item' || !state.selectedItem || state.isPlayerOpen || state.activeTrailer) {
    return undefined;
  }

  if (state.selectedItem.theme_song_url) {
    return {
      kind: 'audio',
      src: resolveApiUrl(state.selectedItem.theme_song_url),
      title: state.selectedItem.display_title,
    };
  }

  const youtubeUrl = state.selectedItem.theme_song_youtube_url
    ? buildYouTubeEmbedUrl(state.selectedItem.theme_song_youtube_url, { autoplay: true, controls: false })
    : undefined;
  if (!youtubeUrl) {
    return undefined;
  }

  return {
    kind: 'youtube',
    src: youtubeUrl,
    title: state.selectedItem.display_title,
  };
}

function syncThemeSongPlayer(): void {
  const layer = themeSongLayer();
  const source = currentThemeSongSource();
  if (!source) {
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
    layer.innerHTML = `
      <iframe
        id="theme-song-youtube-player"
        class="theme-song-iframe"
        src="${escapeHtml(source.src)}"
        title="${escapeHtml(source.title)} theme song"
        allow="autoplay; encrypted-media; picture-in-picture"
        referrerpolicy="origin"
        tabindex="-1"
      ></iframe>
    `;
    return;
  }

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

function bindPlayerProgress(): void {
  const player = document.querySelector<HTMLMediaElement>('#media-player');
  if (!player || !state.selectedItem) {
    return;
  }

  let lastSentSeconds = -1;
  player.addEventListener('timeupdate', () => {
    const currentSeconds = Math.floor(player.currentTime);
    if (currentSeconds === lastSentSeconds || currentSeconds % 15 !== 0) {
      return;
    }

    lastSentSeconds = currentSeconds;
    void updatePlaybackProgress(state.selectedItem!.id, {
      position_ms: Math.floor(player.currentTime * 1000),
      duration_ms: Number.isFinite(player.duration) ? Math.floor(player.duration * 1000) : state.selectedItem?.duration_ms,
      completed: false,
    });
  });

  player.addEventListener('ended', () => {
    void updatePlaybackProgress(state.selectedItem!.id, {
      position_ms: Math.floor((Number.isFinite(player.duration) ? player.duration : 0) * 1000),
      duration_ms: Number.isFinite(player.duration) ? Math.floor(player.duration * 1000) : state.selectedItem?.duration_ms,
      completed: true,
    });
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

  document.querySelector<HTMLFormElement>('#search-form')?.addEventListener('submit', (event) => {
    event.preventDefault();
    const input = document.querySelector<HTMLInputElement>('#search-input');
    state.searchQuery = input?.value.trim() ?? '';
    void refreshData();
  });

  document.querySelector<HTMLButtonElement>('#reset-search')?.addEventListener('click', () => {
    state.searchQuery = '';
    void refreshData();
  });

  document.querySelector<HTMLButtonElement>('#refresh-active-library-metadata')?.addEventListener('click', async () => {
    const library = activeLibrary();
    if (!library || libraryRefreshProgress(library)) {
      return;
    }

    try {
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
      if (!nextTab || state.homeTab === nextTab) {
        return;
      }

      state.homeTab = nextTab;
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

      applyBrowseFilter({
        kind: 'category',
        label: category.genre,
        itemIds: category.items.map((item) => item.id),
      });
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

      applyBrowseFilter({
        kind: 'collection',
        label: collection.name,
        itemIds: collection.item_ids,
      });
    });
  });

  document.querySelector<HTMLButtonElement>('#clear-browse-filter')?.addEventListener('click', () => {
    state.browseFilter = undefined;
    render();
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

  document.querySelector<HTMLButtonElement>('#back-to-library')?.addEventListener('click', () => {
    navigateTo(backNavigationTarget().path);
  });

  document.querySelector<HTMLFormElement>('#metadata-search-form')?.addEventListener('submit', async (event) => {
    event.preventDefault();
    if (!state.selectedItem) {
      return;
    }

    const input = document.querySelector<HTMLInputElement>('#metadata-search-input');
    state.metadataSearchQuery = input?.value.trim() ?? '';
    try {
      state.metadataSearchResults = await searchItemMetadata(state.selectedItem.id, state.metadataSearchQuery);
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
    state.activeTrailer = undefined;
    render();
  });

  document.querySelector<HTMLButtonElement>('#play-selected-item')?.addEventListener('click', () => {
    if (!state.selectedPlayback?.can_direct_play) {
      return;
    }

    state.isPlayerOpen = true;
    render();
  });

  document.querySelector<HTMLButtonElement>('#close-player')?.addEventListener('click', () => {
    state.isPlayerOpen = false;
    render();
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

  const addLibraryKindSelect = document.querySelector<HTMLSelectElement>('#add-library-form select[name="library_kind"]');
  addLibraryKindSelect?.addEventListener('change', () => syncAddLibraryProviderOptions());
  syncAddLibraryProviderOptions();

  bindPlayerProgress();
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
    const label = input.closest('label');
    input.disabled = !supported;
    if (!supported) {
      input.checked = false;
    }
    label?.classList.toggle('is-hidden', !supported);
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
}

window.addEventListener('popstate', () => {
  state.route = parseRoute();
  if (state.route.page === 'home') {
    state.homeTab = defaultHomeTab(state.route);
    state.browseFilter = undefined;
  }
  state.isTrailerMenuOpen = false;
  void refreshData();
});

render();
void refreshData();

