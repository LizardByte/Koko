import {
  addMockLibrary,
  createMockUser,
  getMockCapabilities,
  getMockBootstrap,
  getMockHome,
  getMockItems,
  getMockItem,
  getMockItemMetadata,
  getMockLibraries,
  getMockMetadataProviders,
  getMockLogs,
  getMockUsers,
  getMockPlayback,
  getMockSystemActivities,
  refreshMockLibraryMetadata,
  refreshMockItemMetadata,
  getMockSettings,
  linkMockItemMetadata,
  loginMockUser,
  removeMockLibrary,
  searchMockItemMetadata,
  searchMockItems,
  updateMockUser,
  updateMockPlaybackProgress,
  updateMockSettings,
} from './mockApi';

const REQUEST_TIMEOUT_MS = 15000;

export interface ServerCapabilities {
  app_name: string;
  version: string;
  server_url: string;
  https_enabled: boolean;
  libraries_configured: number;
  api_versions: string[];
  transcoding: {
    ffmpeg: {
      available: boolean;
      version?: string;
      error?: string;
    };
    ffprobe: {
      available: boolean;
      version?: string;
      error?: string;
    };
  };
}

export interface BootstrapUser {
  id: number;
  username: string;
  admin: boolean;
  birthday?: string;
  profile_image_url?: string;
  preferred_metadata_languages: string[];
}

export interface AppBootstrapResponse {
  has_users: boolean;
  current_user?: BootstrapUser;
}

export interface LoginRequest {
  username: string;
  password: string;
}

export interface TokenResponse {
  token: string;
}

export interface CreateUserRequest {
  username: string;
  password: string;
  pin?: string;
  admin: boolean;
  birthday?: string;
  profile_image_url?: string;
  preferred_metadata_languages?: string[];
}

export interface UpdateUserRequest {
  username: string;
  admin: boolean;
  birthday?: string;
  profile_image_url?: string;
  preferred_metadata_languages?: string[];
}

export interface MediaLibrary {
  id: number;
  name: string;
  path: string;
  paths: string[];
  recursive: boolean;
  kind: string;
  status: string;
  scan_revision: number;
  last_scanned_at?: number;
  total_files: number;
  video_files: number;
  audio_files: number;
  image_files: number;
  book_files: number;
  other_files: number;
  metadata_refresh_total: number;
  metadata_refresh_pending: number;
  metadata_refresh_completed: number;
  metadata_refresh_failed: number;
  error?: string;
}

export interface MediaItemSummary {
  id: number;
  library_id: number;
  parent_id?: number | null;
  item_type: string;
  display_title: string;
  relative_path: string;
  media_kind: string;
  playable: boolean;
  child_count: number;
  season_number?: number;
  episode_number?: number;
  duration_ms?: number;
  width?: number;
  height?: number;
  genres: string[];
  has_metadata?: boolean;
  metadata_refresh_state?: string;
  metadata_refresh_error?: string;
  artwork_updated_at?: number;
  modified_at?: number;
}

export interface MediaItemDetail extends MediaItemSummary {
  file_size?: number;
  container?: string;
  bit_rate?: number;
  video_codec?: string;
  audio_codec?: string;
  metadata_json?: string;
  metadata_updated_at?: number;
  poster_url?: string;
  backdrop_url?: string;
  theme_song_url?: string;
  theme_song_youtube_url?: string;
  tagline?: string;
  overview?: string;
  genres: string[];
  release_year?: number;
  logo_url?: string;
  rating?: number;
  content_rating?: string;
  linked_media_type?: string;
  trailer_title?: string;
  trailer_url?: string;
  artwork_updated_at?: number;
  subtitle_tracks: MediaSubtitleTrack[];
  hierarchy: MediaItemSummary[];
  children: MediaItemSummary[];
}

export interface MediaSubtitleTrack {
  index: number;
  label: string;
  format: string;
  url: string;
}

export interface MetadataProviderStatus {
  id: string;
  display_name: string;
  description: string;
  supported_kinds: string[];
  requires_api_key: boolean;
  implemented: boolean;
  enabled: boolean;
  configured: boolean;
  language: string;
  attribution_text: string;
  attribution_url: string;
  logo_light_url?: string;
  logo_dark_url?: string;
}

export interface ItemMetadataMatch {
  id: number;
  provider_id: string;
  external_id: string;
  title?: string;
  overview?: string;
  artwork_url?: string;
  backdrop_url?: string;
  release_year?: number;
  media_type?: string;
  match_state: string;
  provider_payload_json?: string;
  locale_key: string;
  provider_locale_key?: string;
  cached_artwork_path?: string;
  cached_backdrop_path?: string;
  refresh_state?: string;
  last_refreshed_at?: number;
  next_refresh_at?: number;
  refresh_error?: string;
  updated_at?: number;
}

export interface ItemMetadataResponse {
  item_id: number;
  providers: MetadataProviderStatus[];
  matches: ItemMetadataMatch[];
}

export interface MetadataSearchResult {
  provider_id: string;
  external_id: string;
  media_type: string;
  title: string;
  overview?: string;
  artwork_url?: string;
  backdrop_url?: string;
  release_year?: number;
  score?: number;
}

export interface MediaShelf {
  id: string;
  title: string;
  items: MediaItemSummary[];
}

export interface MediaCollectionSummary {
  id: string;
  provider_id: string;
  external_id: string;
  name: string;
  overview?: string;
  artwork_url?: string;
  backdrop_url?: string;
  item_ids: number[];
  item_count: number;
}

export interface MediaHome {
  library_id?: number;
  shelves: MediaShelf[];
  collections: MediaCollectionSummary[];
}

export interface PlaybackDecision {
  item_id: number;
  can_direct_play: boolean;
  transcode_required: boolean;
  reason: string;
  stream_url?: string;
  mime_type?: string;
}

export interface MetadataProviderSettings {
  id: string;
  enabled: boolean;
  api_key?: string | null;
  language: string;
  rate_limit_per_second: number;
  retry_attempts: number;
  retry_backoff_ms: number;
}

export interface SystemActivity {
  id: string;
  category: string;
  scope: string;
  source: string;
  state: string;
  label: string;
  provider_id?: string;
  library_id?: number;
  root_item_id?: number;
  item_ids: number[];
  total_items: number;
  completed_items: number;
  failed_items: number;
  queued_at: number;
  started_at?: number;
  updated_at: number;
}

export interface SystemActivitiesResponse {
  generated_at: number;
  activities: SystemActivity[];
}

export interface LogEntry {
  timestamp: string;
  level: string;
  module: string;
  source_file_path: string;
  line_number?: number;
  message: string;
}

export interface LogEntriesResponse {
  log_path: string;
  entries: LogEntry[];
}

export interface MediaLibrarySettings {
  name: string;
  path: string;
  paths: string[];
  recursive: boolean;
  kind: string;
  metadata_providers: string[];
}

export interface SettingsSnapshot {
  general: {
    data_dir: string;
  };
  media: {
    libraries: MediaLibrarySettings[];
  };
  metadata: {
    providers: MetadataProviderSettings[];
    refresh_interval_days?: number | null;
  };
  server: {
    use_https: boolean;
    address: string;
    port: number;
    cert_path: string;
    key_path: string;
    use_custom_certs: boolean;
  };
  ffmpeg: {
    ffmpeg_path: string;
    ffprobe_path: string;
  };
}

export interface SettingsResponse {
  settings: SettingsSnapshot;
  settings_path: string;
}

export interface PlaybackProgressRequest {
  position_ms: number;
  duration_ms?: number;
  completed: boolean;
}

export interface LinkMetadataRequest {
  provider_id: string;
  external_id: string;
  media_type: string;
}

export type ApiMode = 'live' | 'mock';

const LOCAL_STORAGE_KEY = 'koko-client-web-api-base';
const AUTH_TOKEN_STORAGE_KEY = 'koko-client-web-auth-token';
const ENV_API_BASE_URL = import.meta.env.VITE_API_BASE_URL?.trim();
const ENV_USE_MOCK_API = import.meta.env.VITE_USE_MOCK_API === 'true';
let activeApiMode: ApiMode = ENV_USE_MOCK_API ? 'mock' : 'live';

export function getStoredApiBase(): string {
  if (ENV_API_BASE_URL) {
    return ENV_API_BASE_URL.replace(/\/$/, '');
  }

  const stored = window.localStorage.getItem(LOCAL_STORAGE_KEY)?.trim();
  if (stored) {
    return stored.replace(/\/$/, '');
  }

  return window.location.origin.replace(/\/$/, '');
}

export function getStoredAuthToken(): string | undefined {
  return window.localStorage.getItem(AUTH_TOKEN_STORAGE_KEY)?.trim() || undefined;
}

export function setStoredAuthToken(token: string): void {
  window.localStorage.setItem(AUTH_TOKEN_STORAGE_KEY, token.trim());
}

export function clearStoredAuthToken(): void {
  window.localStorage.removeItem(AUTH_TOKEN_STORAGE_KEY);
}

export function getApiMode(): ApiMode {
  return activeApiMode;
}

function shouldUseMockApi(): boolean {
  return ENV_USE_MOCK_API || activeApiMode === 'mock';
}

function useLiveApi(): void {
  activeApiMode = 'live';
}

function useMockApi(): void {
  activeApiMode = 'mock';
}

function getMockJsonResponse<T>(method: string, path: string, body?: unknown): T {
  const url = new URL(path, 'http://koko.local');

  if (method === 'GET') {
    switch (url.pathname) {
      case '/api/v1/system/capabilities':
        return getMockCapabilities() as T;
      case '/api/v1/bootstrap':
        return getMockBootstrap() as T;
      case '/api/v1/users':
        return getMockUsers() as T;
      case '/api/v1/libraries':
        return getMockLibraries() as T;
      case '/api/v1/metadata/providers':
        return getMockMetadataProviders() as T;
      case '/api/v1/system/activities':
        return getMockSystemActivities() as T;
      case '/api/v1/settings':
        return getMockSettings() as T;
      case '/api/v1/settings/logs':
        return getMockLogs(
          url.searchParams.get('level') ?? undefined,
          url.searchParams.get('module') ?? undefined,
          url.searchParams.get('search') ?? undefined,
          url.searchParams.get('since') ?? undefined,
          url.searchParams.get('until') ?? undefined,
          url.searchParams.get('limit') ? Number(url.searchParams.get('limit')) : undefined,
        ) as T;
      case '/api/v1/home': {
        const libraryId = url.searchParams.get('library_id');
        return getMockHome(libraryId ? Number(libraryId) : undefined) as T;
      }
      case '/api/v1/items': {
        const libraryId = url.searchParams.get('library_id');
        return getMockItems(libraryId ? Number(libraryId) : undefined) as T;
      }
      case '/api/v1/search': {
        const query = url.searchParams.get('query') ?? '';
        const libraryId = url.searchParams.get('library_id');
        return searchMockItems(query, libraryId ? Number(libraryId) : undefined) as T;
      }
      default: {
        const itemMetadataSearchMatch = url.pathname.match(/^\/api\/v1\/items\/(\d+)\/metadata\/search$/);
        if (itemMetadataSearchMatch) {
          return searchMockItemMetadata(
            Number(itemMetadataSearchMatch[1]),
            url.searchParams.get('query') ?? undefined,
          ) as T;
        }

        const itemMetadataMatch = url.pathname.match(/^\/api\/v1\/items\/(\d+)\/metadata$/);
        if (itemMetadataMatch) {
          const itemMetadata = getMockItemMetadata(Number(itemMetadataMatch[1]));
          if (!itemMetadata) {
            throw new Error('404 Not Found');
          }

          return itemMetadata as T;
        }

        const itemPlaybackMatch = url.pathname.match(/^\/api\/v1\/items\/(\d+)\/playback$/);
        if (itemPlaybackMatch) {
          return getMockPlayback(Number(itemPlaybackMatch[1])) as T;
        }

        const itemMatch = url.pathname.match(/^\/api\/v1\/items\/(\d+)$/);
        if (itemMatch) {
          const item = getMockItem(Number(itemMatch[1]));
          if (!item) {
            throw new Error('404 Not Found');
          }

          return item as T;
        }

        throw new Error(`No mock response is defined for ${method} ${url.pathname}`);
      }
    }
  }

  if (method === 'PUT' && url.pathname === '/api/v1/settings') {
    return updateMockSettings(body as SettingsSnapshot) as T;
  }

  const updateUserMatch = url.pathname.match(/^\/api\/v1\/users\/(\d+)$/);
  if (method === 'PUT' && updateUserMatch) {
    return updateMockUser(Number(updateUserMatch[1]), body as UpdateUserRequest) as T;
  }

  if (method === 'POST' && url.pathname === '/login') {
    return loginMockUser(body as LoginRequest) as T;
  }

  if (method === 'POST' && url.pathname === '/create_user') {
    return createMockUser(body as CreateUserRequest) as T;
  }

  if (method === 'POST' && url.pathname === '/api/v1/settings/libraries') {
    return addMockLibrary(body as { library: MediaLibrarySettings }) as T;
  }

  const removeLibraryMatch = url.pathname.match(/^\/api\/v1\/settings\/libraries\/(\d+)$/);
  if (method === 'DELETE' && removeLibraryMatch) {
    return removeMockLibrary(Number(removeLibraryMatch[1])) as T;
  }

  const itemProgressMatch = url.pathname.match(/^\/api\/v1\/items\/(\d+)\/progress$/);
  if (method === 'POST' && itemProgressMatch) {
    updateMockPlaybackProgress(Number(itemProgressMatch[1]), body as PlaybackProgressRequest);
    return undefined as T;
  }

  const itemLinkMatch = url.pathname.match(/^\/api\/v1\/items\/(\d+)\/metadata\/link$/);
  if (method === 'POST' && itemLinkMatch) {
    return linkMockItemMetadata(Number(itemLinkMatch[1]), body as LinkMetadataRequest) as T;
  }

  const itemRefreshMatch = url.pathname.match(/^\/api\/v1\/items\/(\d+)\/metadata\/refresh$/);
  if (method === 'POST' && itemRefreshMatch) {
    return refreshMockItemMetadata(Number(itemRefreshMatch[1])) as T;
  }

  const libraryRefreshMatch = url.pathname.match(/^\/api\/v1\/libraries\/(\d+)\/metadata\/refresh$/);
  if (method === 'POST' && libraryRefreshMatch) {
    return refreshMockLibraryMetadata(Number(libraryRefreshMatch[1])) as T;
  }

  const libraryScanMatch = url.pathname.match(/^\/api\/v1\/libraries\/(\d+)\/scan$/);
  if (method === 'POST' && libraryScanMatch) {
    return refreshMockLibraryMetadata(Number(libraryScanMatch[1])) as T;
  }

  throw new Error(`No mock response is defined for ${method} ${url.pathname}`);
}

async function requestJson<T>(method: string, path: string, body?: unknown): Promise<T> {
  if (shouldUseMockApi()) {
    useMockApi();
    return getMockJsonResponse<T>(method, path, body);
  }

  try {
    const abortController = new AbortController();
    const timeoutHandle = window.setTimeout(() => abortController.abort(), REQUEST_TIMEOUT_MS);
    const response = await fetch(`${getStoredApiBase()}${path}`, {
      method,
      headers: {
        ...(body === undefined ? {} : { 'Content-Type': 'application/json' }),
        ...(getStoredAuthToken() ? { Authorization: `Bearer ${getStoredAuthToken()}` } : {}),
      },
      body: body === undefined ? undefined : JSON.stringify(body),
      signal: abortController.signal,
    }).finally(() => {
      window.clearTimeout(timeoutHandle);
    });
    if (!response.ok) {
      if (response.status === 401) {
        clearStoredAuthToken();
      }
      const responseText = (await response.text()).trim();
      const error = new Error(
        responseText
          ? `${response.status} ${response.statusText}: ${responseText}`
          : `${response.status} ${response.statusText}`,
      );
      if (import.meta.env.DEV) {
        useMockApi();
        return getMockJsonResponse<T>(method, path, body);
      }

      return Promise.reject(error);
    }

    useLiveApi();
    if (response.status === 204) {
      return undefined as T;
    }
    if (response.headers.get('content-type')?.includes('application/json')) {
      return response.json() as Promise<T>;
    }

    return undefined as T;
  } catch (error) {
    if (error instanceof DOMException && error.name === 'AbortError') {
      throw new Error(`Request timed out after ${REQUEST_TIMEOUT_MS / 1000} seconds.`);
    }
    if (import.meta.env.DEV) {
      useMockApi();
      return getMockJsonResponse<T>(method, path, body);
    }

    throw error;
  }
}

export function getCapabilities(): Promise<ServerCapabilities> {
  return requestJson<ServerCapabilities>('GET', '/api/v1/system/capabilities');
}

export function getAppBootstrap(): Promise<AppBootstrapResponse> {
  return requestJson<AppBootstrapResponse>('GET', '/api/v1/bootstrap');
}

export function loginUser(request: LoginRequest): Promise<TokenResponse> {
  return requestJson<TokenResponse>('POST', '/login', request);
}

export function createUser(request: CreateUserRequest): Promise<string> {
  return requestJson<string>('POST', '/create_user', request);
}

export function getUsers(): Promise<BootstrapUser[]> {
  return requestJson<BootstrapUser[]>('GET', '/api/v1/users');
}

export function updateUser(userId: number, request: UpdateUserRequest): Promise<BootstrapUser> {
  return requestJson<BootstrapUser>('PUT', `/api/v1/users/${userId}`, request);
}

export function getLibraries(): Promise<MediaLibrary[]> {
  return requestJson<MediaLibrary[]>('GET', '/api/v1/libraries');
}

export function getHome(libraryId?: number): Promise<MediaHome> {
  const query = typeof libraryId === 'number' ? `?library_id=${libraryId}` : '';
  return requestJson<MediaHome>('GET', `/api/v1/home${query}`);
}

export function getItems(libraryId?: number): Promise<MediaItemSummary[]> {
  const query = typeof libraryId === 'number' ? `?library_id=${libraryId}` : '';
  return requestJson<MediaItemSummary[]>('GET', `/api/v1/items${query}`);
}

export function searchItems(query: string, libraryId?: number): Promise<MediaItemSummary[]> {
  const params = new URLSearchParams({ query });
  if (typeof libraryId === 'number') {
    params.set('library_id', String(libraryId));
  }

  return requestJson<MediaItemSummary[]>('GET', `/api/v1/search?${params.toString()}`);
}

export function getItem(itemId: number): Promise<MediaItemDetail> {
  return requestJson<MediaItemDetail>('GET', `/api/v1/items/${itemId}`);
}

export function getMetadataProviders(): Promise<MetadataProviderStatus[]> {
  return requestJson<MetadataProviderStatus[]>('GET', '/api/v1/metadata/providers');
}

export function getSystemActivities(): Promise<SystemActivitiesResponse> {
  return requestJson<SystemActivitiesResponse>('GET', '/api/v1/system/activities');
}

export function getItemMetadata(itemId: number): Promise<ItemMetadataResponse> {
  return requestJson<ItemMetadataResponse>('GET', `/api/v1/items/${itemId}/metadata`);
}

export interface MetadataSearchOptions {
  query?: string;
  providers?: string[];
  year?: string;
  language?: string;
}

export function searchItemMetadata(itemId: number, options?: string | MetadataSearchOptions): Promise<MetadataSearchResult[]> {
  const params = new URLSearchParams();
  const normalizedOptions = typeof options === 'string' ? { query: options } : options;
  if (normalizedOptions?.query?.trim()) {
    params.set('query', normalizedOptions.query.trim());
  }
  if (normalizedOptions?.providers?.length) {
    params.set('providers', normalizedOptions.providers.join(','));
  }
  if (normalizedOptions?.year?.trim()) {
    params.set('year', normalizedOptions.year.trim());
  }
  if (normalizedOptions?.language?.trim()) {
    params.set('language', normalizedOptions.language.trim());
  }
  const suffix = params.toString() ? `?${params.toString()}` : '';
  return requestJson<MetadataSearchResult[]>('GET', `/api/v1/items/${itemId}/metadata/search${suffix}`);
}

export function linkItemMetadata(itemId: number, request: LinkMetadataRequest): Promise<ItemMetadataMatch> {
  return requestJson<ItemMetadataMatch>('POST', `/api/v1/items/${itemId}/metadata/link`, request);
}

export function refreshItemMetadata(itemId: number): Promise<ItemMetadataMatch> {
  return requestJson<ItemMetadataMatch>('POST', `/api/v1/items/${itemId}/metadata/refresh`);
}

export function refreshLibraryMetadata(libraryId: number): Promise<MediaLibrary> {
  return requestJson<MediaLibrary>('POST', `/api/v1/libraries/${libraryId}/metadata/refresh`);
}

export function scanLibrary(libraryId: number): Promise<MediaLibrary> {
  return requestJson<MediaLibrary>('POST', `/api/v1/libraries/${libraryId}/scan`);
}

export function getPlaybackDecision(itemId: number): Promise<PlaybackDecision> {
  return requestJson<PlaybackDecision>('GET', `/api/v1/items/${itemId}/playback`);
}

export function updatePlaybackProgress(itemId: number, request: PlaybackProgressRequest): Promise<void> {
  return requestJson<void>('POST', `/api/v1/items/${itemId}/progress`, request);
}

export function getSettings(): Promise<SettingsResponse> {
  return requestJson<SettingsResponse>('GET', '/api/v1/settings');
}

export function getLogs(filters?: {
  level?: string;
  module?: string;
  search?: string;
  since?: string;
  until?: string;
  limit?: number;
}): Promise<LogEntriesResponse> {
  const params = new URLSearchParams();
  if (filters?.level?.trim()) {
    params.set('level', filters.level.trim());
  }
  if (filters?.module?.trim()) {
    params.set('module', filters.module.trim());
  }
  if (filters?.search?.trim()) {
    params.set('search', filters.search.trim());
  }
  if (filters?.since?.trim()) {
    params.set('since', filters.since.trim());
  }
  if (filters?.until?.trim()) {
    params.set('until', filters.until.trim());
  }
  if (typeof filters?.limit === 'number' && Number.isFinite(filters.limit)) {
    params.set('limit', String(filters.limit));
  }

  const suffix = params.toString() ? `?${params.toString()}` : '';
  return requestJson<LogEntriesResponse>('GET', `/api/v1/settings/logs${suffix}`);
}

export function updateSettings(settings: SettingsSnapshot): Promise<SettingsResponse> {
  return requestJson<SettingsResponse>('PUT', '/api/v1/settings', settings);
}

export function addLibrary(library: MediaLibrarySettings): Promise<SettingsResponse> {
  return requestJson<SettingsResponse>('POST', '/api/v1/settings/libraries', { library });
}

export function deleteLibrary(libraryIndex: number): Promise<SettingsResponse> {
  return requestJson<SettingsResponse>('DELETE', `/api/v1/settings/libraries/${libraryIndex}`);
}

export function resolveApiUrl(path: string): string {
  if (/^https?:\/\//i.test(path)) {
    return path;
  }

  const normalizedPath = path.startsWith('/') ? path : `/${path}`;
  return `${getStoredApiBase()}${normalizedPath}`;
}

export function getStreamUrl(itemId: number): string {
  return `${getStoredApiBase()}/api/v1/items/${itemId}/stream`;
}

export function getArtworkUrl(itemId: number, kind: 'poster' | 'backdrop' = 'poster', revision?: number): string {
  const params = new URLSearchParams({ kind });
  if (typeof revision === 'number') {
    params.set('rev', String(revision));
  }

  return `${getStoredApiBase()}/api/v1/items/${itemId}/artwork?${params.toString()}`;
}
