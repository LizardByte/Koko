// Data layer for the Svelte client — full-fidelity port of
// ../client-web/src/api.ts. Types, request shapes, URL builders, and the
// VITE_USE_MOCK_API toggle all mirror the vanilla client. The mock dispatch
// layer lives in ./mockApi.ts.

// ---------------------------------------------------------------------------
// Auth / bootstrap
// ---------------------------------------------------------------------------

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

export interface ProfileImageUploadRequest {
  mime_type: string;
  data_base64: string;
}

export interface LoginRequest {
  username: string;
  password: string;
}

export interface CreateUserRequest {
  username: string;
  password: string;
  pin?: string;
  admin: boolean;
  birthday?: string;
  profile_image_upload?: ProfileImageUploadRequest;
  preferred_metadata_languages?: string[];
}

export interface UpdateUserRequest {
  username: string;
  admin: boolean;
  birthday?: string;
  profile_image_upload?: ProfileImageUploadRequest;
  remove_profile_image?: boolean;
  preferred_metadata_languages?: string[];
}

export interface TokenResponse {
  token: string;
}

// ---------------------------------------------------------------------------
// System / capabilities
// ---------------------------------------------------------------------------

export interface ServerCapabilities {
  app_name: string;
  version: string;
  server_url: string;
  https_enabled: boolean;
  libraries_configured: number;
  api_versions: string[];
  transcoding: {
    ffmpeg: { available: boolean; version?: string; error?: string };
    ffprobe: { available: boolean; version?: string; error?: string };
  };
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

// ---------------------------------------------------------------------------
// Libraries
// ---------------------------------------------------------------------------

export interface MediaLibrary {
  id: number;
  name: string;
  path: string;
  paths: string[];
  recursive: boolean;
  kind: string;
  scanner: string;
  metadata_providers: string[];
  metadata_language_mode: 'auto' | 'manual';
  metadata_languages: string[];
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
  missing_files: number;
  missing_items: number;
  error?: string;
}

export interface MediaLibrarySettings {
  name: string;
  path: string;
  paths: string[];
  recursive: boolean;
  kind: string;
  scanner: string;
  metadata_providers: string[];
  metadata_language_mode: 'auto' | 'manual';
  metadata_languages: string[];
  allowed_user_ids: number[];
}

// ---------------------------------------------------------------------------
// Items
// ---------------------------------------------------------------------------

export interface MediaItemSummary {
  id: number;
  library_id: number;
  parent_id?: number | null;
  item_type: string;
  display_title: string;
  display_subtitle?: string | null;
  artwork_item_id?: number | null;
  relative_path: string;
  media_kind: string;
  playable: boolean;
  child_count: number;
  available_season_count?: number | null;
  season_number?: number;
  episode_number?: number;
  duration_ms?: number;
  width?: number;
  height?: number;
  genres: string[];
  overview?: string;
  backdrop_url?: string;
  logo_url?: string;
  has_metadata?: boolean;
  metadata_refresh_state?: string;
  metadata_refresh_error?: string;
  artwork_updated_at?: number;
  modified_at?: number;
  playback_position_ms?: number;
  playback_duration_ms?: number;
  playback_completed?: boolean;
  watch_count?: number;
  last_watched_at?: number | null;
  missing_since?: number | null;
  hierarchy?: MediaItemSummary[];
}

export interface MediaPlaybackTarget {
  item_id: number;
  start_ms: number;
  label: string;
  display_title: string;
  season_number?: number;
  episode_number?: number;
  resume: boolean;
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
  tagline?: string;
  overview?: string;
  release_year?: number;
  logo_url?: string;
  rating?: number;
  content_rating?: string;
  linked_media_type?: string;
  trailer_title?: string;
  trailer_url?: string;
  extras: MediaItemExtra[];
  playback_target?: MediaPlaybackTarget | null;
  restart_playback_target?: MediaPlaybackTarget | null;
  audio_tracks: MediaAudioTrack[];
  subtitle_tracks: MediaSubtitleTrack[];
  hierarchy: MediaItemSummary[];
  children: MediaItemSummary[];
}

export interface MediaItemExtra {
  extra_type: string;
  title?: string;
  url: string;
  duration_seconds?: number;
  thumbnail_url?: string;
}

export interface MediaAudioTrack {
  index: number;
  label: string;
  codec?: string;
  language?: string;
  default: boolean;
}

export interface MediaSubtitleTrack {
  index: number;
  label: string;
  format: string;
  url: string;
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
  theme_song_url?: string;
  item_ids: number[];
  item_count: number;
}

export interface MediaHome {
  library_id?: number;
  shelves: MediaShelf[];
  collections: MediaCollectionSummary[];
}

export interface MediaPlaylistSearchSummary {
  id: string;
  name: string;
  overview?: string;
  item_count: number;
}

export type MediaSearchResult =
  | { result_type: 'item'; item: MediaItemSummary }
  | { result_type: 'collection'; collection: MediaCollectionSummary }
  | { result_type: 'person'; person: MetadataPersonSummary }
  | { result_type: 'playlist'; playlist: MediaPlaylistSearchSummary };

// ---------------------------------------------------------------------------
// Metadata
// ---------------------------------------------------------------------------

export interface MetadataProviderStatus {
  id: string;
  display_name: string;
  description: string;
  supported_kinds: string[];
  requires_api_key: boolean;
  implemented: boolean;
  role: 'primary' | 'secondary';
  extends_provider_ids: string[];
  enabled: boolean;
  configured: boolean;
  language: string;
  attribution_text: string;
  attribution_url: string;
  logo_light_url?: string;
  logo_dark_url?: string;
}

export interface MetadataProviderSettings {
  id: string;
  enabled: boolean;
  api_key?: string | null;
  api_key_secret_ref?: string;
  api_key_configured?: boolean;
  clear_api_key?: boolean;
  language: string;
  rate_limit_per_second: number;
  retry_attempts: number;
  retry_backoff_ms: number;
}

export interface ItemMetadataPerson {
  id: number;
  person_id: number;
  external_id?: string;
  locale_key?: string;
  name: string;
  role?: string;
  department?: string;
  character_name?: string;
  profile_url?: string;
  image_url?: string;
  cached_image_path?: string;
  sort_order: number;
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
  relation_kind: string;
  match_state: string;
  logo_url?: string;
  cached_logo_path?: string;
  genres: string[];
  people: ItemMetadataPerson[];
  rating?: number;
  content_rating?: string;
  trailer_title?: string;
  trailer_url?: string;
  theme_song_url?: string;
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

export interface MetadataSearchOptions {
  query?: string;
  providers?: string[];
  year?: string;
  language?: string;
}

export interface LinkMetadataRequest {
  provider_id: string;
  external_id: string;
  media_type: string;
}

// ---------------------------------------------------------------------------
// People
// ---------------------------------------------------------------------------

export interface MetadataPersonSummary {
  id: number;
  provider_id: string;
  external_id?: string;
  locale_key: string;
  name: string;
  known_for: string[];
  biography?: string;
  gender?: string;
  birthday?: string;
  deathday?: string;
  birth_place?: string;
  profile_url?: string;
  image_url?: string;
  cached_image_path?: string;
  updated_at?: number;
}

export interface MetadataPersonCreditSummary {
  id: number;
  metadata_link_id: number;
  media_item_id: number;
  role?: string;
  department?: string;
  character_name?: string;
  sort_order: number;
}

export interface MetadataPersonItemCredit {
  credit: MetadataPersonCreditSummary;
  item: MediaItemSummary;
  hierarchy: MediaItemSummary[];
}

export interface MetadataPersonResponse {
  person: MetadataPersonSummary;
  credits: MetadataPersonItemCredit[];
}

// ---------------------------------------------------------------------------
// Playback
// ---------------------------------------------------------------------------

export interface PlaybackDecision {
  item_id: number;
  can_direct_play: boolean;
  transcode_required: boolean;
  reason: string;
  stream_url?: string;
  mime_type?: string;
  transcode_container?: string;
  transcode_video_codec?: string;
  transcode_audio_codec?: string;
  video_transcode_required: boolean;
  audio_transcode_required: boolean;
  source_video_codec?: string;
  source_audio_codec?: string;
  source_container?: string;
}

export interface ClientProfile {
  client_type: string;
  client_name: string;
  supported_containers: string[];
  supported_video_codecs: string[];
  supported_audio_codecs: string[];
  supported_subtitle_formats: string[];
  max_video_width: number;
  max_video_height: number;
  max_bitrate_kbps: number;
  supports_adaptive_streaming: boolean;
  prefer_hls: boolean;
}

export interface CreateSessionRequest {
  item_id: number;
  client_profile: ClientProfile;
}

export interface PlaybackSession {
  session_id: string;
  item_id: number;
  user_id?: number;
  client_profile: ClientProfile;
  decision: PlaybackDecision;
  created_at: number;
  audio_stream_index?: number;
}

export interface PlaybackProgressRequest {
  position_ms: number;
  duration_ms?: number;
  completed: boolean;
}

// ---------------------------------------------------------------------------
// Settings / scheduled tasks / logs
// ---------------------------------------------------------------------------

export type ScheduledTaskWeekday =
  | 'monday'
  | 'tuesday'
  | 'wednesday'
  | 'thursday'
  | 'friday'
  | 'saturday'
  | 'sunday';

export type ScheduledTaskId = 'metadata_refresh' | 'trash_cleanup' | 'database_maintenance';

export interface SettingsSnapshot {
  general: { data_dir: string };
  media: {
    libraries: MediaLibrarySettings[];
    missing_item_auto_delete_days?: number | null;
  };
  metadata: {
    providers: MetadataProviderSettings[];
    refresh_interval_days?: number | null;
  };
  scheduled_tasks: {
    enabled: boolean;
    window: { start_time: string; stop_time: string; weekdays: ScheduledTaskWeekday[] };
    metadata_refresh: { enabled: boolean };
    trash_cleanup: {
      enabled: boolean;
      missing_item_auto_delete_days?: number | null;
      interval_days: number;
    };
    database_maintenance: { enabled: boolean; interval_days: number };
  };
  server: {
    use_https: boolean;
    address: string;
    port: number;
    cert_path: string;
    key_path: string;
    use_custom_certs: boolean;
  };
  ffmpeg: { ffmpeg_path: string; ffprobe_path: string };
}

export interface SettingsResponse {
  settings: SettingsSnapshot;
  settings_path: string;
}

export interface ScheduledTaskRunResponse {
  task_id: ScheduledTaskId;
  started: boolean;
  message: string;
}

export interface MetadataCacheClearResponse {
  removed_files: number;
}

export interface MissingItemsCleanupResponse {
  library_id: number;
  deleted_files: number;
  deleted_items: number;
  removed_collection_items: number;
  library: MediaLibrary;
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

export interface LogFilters {
  level: string;
  module: string;
  search: string;
  since: string;
  until: string;
}

export const EMPTY_LOG_FILTERS: LogFilters = {
  level: '',
  module: '',
  search: '',
  since: '',
  until: '',
};

// ---------------------------------------------------------------------------
// Mock toggle + transport
// ---------------------------------------------------------------------------

export type ApiMode = 'live' | 'mock';

export const AUTH_TOKEN_STORAGE_KEY = 'koko-client-web-auth-token';
const API_BASE_STORAGE_KEY = 'koko-client-web-api-base';

// Same toggle as the vanilla client. `vite dev --mode mock` loads .env.mock.
const ENV_USE_MOCK_API = import.meta.env.VITE_USE_MOCK_API === 'true';

// activeApiMode is module state so the dev-mode silent fallback (see
// requestJson) can flip the whole session to mock after a live failure, and a
// live success can flip it back — matching the vanilla client's behavior.
let activeApiMode: ApiMode = ENV_USE_MOCK_API ? 'mock' : 'live';

export function isMockApi(): boolean {
  return activeApiMode === 'mock';
}

export function getApiMode(): ApiMode {
  return activeApiMode;
}

export function getStoredApiBase(): string {
  const fromEnv = (import.meta.env.VITE_API_BASE_URL as string | undefined)?.trim();
  if (fromEnv) {
    return fromEnv;
  }
  const stored = globalThis.localStorage?.getItem(API_BASE_STORAGE_KEY)?.trim();
  if (stored) {
    return stored;
  }
  return globalThis.location.origin;
}

export function getStoredAuthToken(): string | undefined {
  return globalThis.localStorage?.getItem(AUTH_TOKEN_STORAGE_KEY)?.trim() || undefined;
}

export function setStoredAuthToken(token: string): void {
  globalThis.localStorage?.setItem(AUTH_TOKEN_STORAGE_KEY, token.trim());
}

export function clearStoredAuthToken(): void {
  globalThis.localStorage?.removeItem(AUTH_TOKEN_STORAGE_KEY);
}

async function requestJson<T>(method: string, path: string, body?: unknown): Promise<T> {
  // Mock-first: if the env flag is set, always serve mock.
  if (activeApiMode === 'mock') {
    return (await import('./mockApi')).dispatch<T>(method, path, body);
  }

  const headers: Record<string, string> = {};
  if (body !== undefined) {
    headers['Content-Type'] = 'application/json';
  }
  const token = getStoredAuthToken();
  if (token) {
    headers.Authorization = `Bearer ${token}`;
  }

  let response: Response;
  response = await fetch(getStoredApiBase() + path, {
    method,
    headers,
    body: body !== undefined ? JSON.stringify(body) : undefined,
  });

  if (!response.ok) {
    if (response.status === 401) {
      clearStoredAuthToken();
    }
    throw new Error(`${method} ${path} failed: ${response.status} ${response.statusText}`);
  }

  // A live success re-asserts live mode (can flip back from a prior fallback).
  activeApiMode = 'live';

  if (response.status === 204) {
    return undefined as T;
  }
  return (await response.json()) as T;
}

// ---------------------------------------------------------------------------
// URL builders (synchronous, non-fetch)
// ---------------------------------------------------------------------------

export function resolveApiUrl(path: string): string {
  if (/^https?:\/\//i.test(path)) {
    return path;
  }
  const normalized = path.startsWith('/') ? path : `/${path}`;
  return `${getStoredApiBase()}${normalized}`;
}

export function getArtworkUrl(itemId: number, kind: 'poster' | 'backdrop' | 'logo' = 'poster', revision?: number): string {
  const params = new URLSearchParams({ kind });
  if (typeof revision === 'number') {
    params.set('rev', String(revision));
  }
  if (isMockApi()) {
    // Storybook / mock mode: if a local artwork has been registered for this
    // item id + kind (see storybook/artworks.ts), serve the bundled asset URL.
    // Otherwise fall back to a deterministic mock:// placeholder — components
    // with gradient fallbacks (MediaCard) render cleanly either way; <img>
    // tags without a fallback show broken-image, matching vanilla's mock mode.
    const local = mockArtworkResolver?.(itemId, kind);
    if (local) return local;
    return `mock://artwork/${itemId}/${kind}`;
  }
  return `${getStoredApiBase()}/api/v1/items/${itemId}/artwork?${params.toString()}`;
}

/**
 * Optional resolver hook for mock mode. The Storybook layer sets this to serve
 * bundled CC0 artwork for fixture items (see storybook/artworks.ts). Production
 * never sets it, so it stays a no-op and is tree-shaken from the real bundle.
 */
export type MockArtworkResolver = (itemId: number, kind: 'poster' | 'backdrop' | 'logo') => string | undefined;
let mockArtworkResolver: MockArtworkResolver | undefined;
export function setMockArtworkResolver(resolver: MockArtworkResolver | undefined): void {
  mockArtworkResolver = resolver;
}

export function getStreamUrl(itemId: number): string {
  return `${getStoredApiBase()}/api/v1/items/${itemId}/stream`;
}

export function getSessionStreamUrl(sessionId: string, startMs?: number, audioStreamIndex?: number): string {
  const params = new URLSearchParams();
  if (typeof startMs === 'number') {
    params.set('start_ms', String(startMs));
  }
  if (typeof audioStreamIndex === 'number') {
    params.set('audio_stream_index', String(audioStreamIndex));
  }
  const suffix = params.toString() ? `?${params.toString()}` : '';
  return `${getStoredApiBase()}/api/v1/sessions/${sessionId}/stream${suffix}`;
}

export function getPersonImageUrl(personId: number): string {
  return `${getStoredApiBase()}/api/v1/people/${personId}/image`;
}

// ---------------------------------------------------------------------------
// API surface
// ---------------------------------------------------------------------------

// System / bootstrap
export const getCapabilities = (): Promise<ServerCapabilities> =>
  requestJson('GET', '/api/v1/system/capabilities');
export const getAppBootstrap = (): Promise<AppBootstrapResponse> =>
  requestJson('GET', '/api/v1/bootstrap');
export const loginUser = (request: LoginRequest): Promise<TokenResponse> =>
  requestJson('POST', '/login', request);
export const createUser = (request: CreateUserRequest): Promise<string> =>
  requestJson('POST', '/create_user', request);
export const getUsers = (): Promise<BootstrapUser[]> => requestJson('GET', '/api/v1/users');
export const updateUser = (userId: number, request: UpdateUserRequest): Promise<BootstrapUser> =>
  requestJson('PUT', `/api/v1/users/${userId}`, request);

// Catalog
export const getLibraries = (): Promise<MediaLibrary[]> => requestJson('GET', '/api/v1/libraries');
export const getHome = (libraryId?: number): Promise<MediaHome> => {
  const suffix = typeof libraryId === 'number' ? `?library_id=${libraryId}` : '';
  return requestJson('GET', `/api/v1/home${suffix}`);
};
export const getItems = (libraryId?: number): Promise<MediaItemSummary[]> => {
  const suffix = typeof libraryId === 'number' ? `?library_id=${libraryId}` : '';
  return requestJson('GET', `/api/v1/items${suffix}`);
};
export const getItem = (itemId: number): Promise<MediaItemDetail> =>
  requestJson('GET', `/api/v1/items/${itemId}`);
export const getPerson = (personId: number): Promise<MetadataPersonResponse> =>
  requestJson('GET', `/api/v1/people/${personId}`);

// Search — the vanilla client appends a synthetic Playlists result client-side
// when the query is a substring of 'playlists'. Reproduce that here.
export async function searchItems(query: string): Promise<MediaSearchResult[]> {
  const params = new URLSearchParams({ query });
  const results = await requestJson<MediaSearchResult[]>('GET', `/api/v1/search?${params.toString()}`);
  if (query.trim().toLowerCase() && 'playlists'.includes(query.trim().toLowerCase())) {
    return [
      ...results,
      { result_type: 'playlist', playlist: { id: 'Playlists', name: 'Playlists', item_count: 0 } },
    ];
  }
  return results;
}

// Metadata
export const getMetadataProviders = (): Promise<MetadataProviderStatus[]> =>
  requestJson('GET', '/api/v1/metadata/providers');
export const getItemMetadata = (itemId: number): Promise<ItemMetadataResponse> =>
  requestJson('GET', `/api/v1/items/${itemId}/metadata`);
export function searchItemMetadata(
  itemId: number,
  options?: string | MetadataSearchOptions,
): Promise<MetadataSearchResult[]> {
  const opts: MetadataSearchOptions =
    typeof options === 'string' ? { query: options } : options ?? {};
  const params = new URLSearchParams();
  if (opts.query) params.set('query', opts.query);
  if (opts.providers?.length) params.set('providers', opts.providers.join(','));
  if (opts.year) params.set('year', opts.year);
  if (opts.language) params.set('language', opts.language);
  return requestJson('GET', `/api/v1/items/${itemId}/metadata/search?${params.toString()}`);
}
export const linkItemMetadata = (itemId: number, request: LinkMetadataRequest): Promise<ItemMetadataMatch> =>
  requestJson('POST', `/api/v1/items/${itemId}/metadata/link`, request);
export const refreshItemMetadata = (itemId: number): Promise<ItemMetadataMatch> =>
  requestJson('POST', `/api/v1/items/${itemId}/metadata/refresh`);
export const refreshLibraryMetadata = (libraryId: number): Promise<MediaLibrary> =>
  requestJson('POST', `/api/v1/libraries/${libraryId}/metadata/refresh`);
export const scanLibrary = (libraryId: number): Promise<MediaLibrary> =>
  requestJson('POST', `/api/v1/libraries/${libraryId}/scan`);
export const deleteMissingItems = (libraryId: number): Promise<MissingItemsCleanupResponse> =>
  requestJson('DELETE', `/api/v1/libraries/${libraryId}/missing`);

// Playback
export const getPlaybackDecision = (itemId: number): Promise<PlaybackDecision> =>
  requestJson('GET', `/api/v1/items/${itemId}/playback`);
export const createPlaybackSession = (request: CreateSessionRequest): Promise<PlaybackSession> =>
  requestJson('POST', '/api/v1/sessions', request);
export const deletePlaybackSession = (sessionId: string): Promise<void> =>
  requestJson('DELETE', `/api/v1/sessions/${sessionId}`);
export const updatePlaybackProgress = (
  itemId: number,
  request: PlaybackProgressRequest,
): Promise<void> => requestJson('POST', `/api/v1/items/${itemId}/progress`, request);

// Settings / logs / scheduled tasks
export const getSettings = (): Promise<SettingsResponse> => requestJson('GET', '/api/v1/settings');
export function getLogs(filters?: {
  level?: string;
  module?: string;
  search?: string;
  since?: string;
  until?: string;
  limit?: number;
}): Promise<LogEntriesResponse> {
  const params = new URLSearchParams();
  // Trim-aware string params: set only when non-empty.
  for (const key of ['level', 'module', 'search', 'since', 'until'] as const) {
    const value = filters?.[key]?.trim();
    if (value) params.set(key, value);
  }
  if (typeof filters?.limit === 'number' && Number.isFinite(filters.limit)) {
    params.set('limit', String(filters.limit));
  }
  const suffix = params.toString() ? `?${params.toString()}` : '';
  return requestJson('GET', `/api/v1/settings/logs${suffix}`);
}
export const updateSettings = (settings: SettingsSnapshot): Promise<SettingsResponse> =>
  requestJson('PUT', '/api/v1/settings', settings);
export const clearMetadataCache = (): Promise<MetadataCacheClearResponse> =>
  requestJson('POST', '/api/v1/settings/metadata-cache/clear');
export const runScheduledTask = (taskId: ScheduledTaskId): Promise<ScheduledTaskRunResponse> =>
  requestJson('POST', `/api/v1/scheduled-tasks/${taskId}/run`);
export const addLibrary = (library: MediaLibrarySettings): Promise<SettingsResponse> =>
  requestJson('POST', '/api/v1/settings/libraries', { library });
export const deleteLibrary = (libraryIndex: number): Promise<SettingsResponse> =>
  requestJson('DELETE', `/api/v1/settings/libraries/${libraryIndex}`);
export const getSystemActivities = (): Promise<SystemActivitiesResponse> =>
  requestJson('GET', '/api/v1/system/activities');

// ---------------------------------------------------------------------------
// Browser capability probe (port of getWebClientProfile from vanilla api.ts)
// ---------------------------------------------------------------------------

export function getWebClientProfile(): ClientProfile {
  const video = document.createElement('video');
  const audio = document.createElement('audio');
  const canVideo = (type: string) => video.canPlayType(type) !== '';
  const canAudio = (type: string) => audio.canPlayType(type) !== '';
  // Codec → MP4 FourCC aliases for canPlayType probing.
  const CODEC_ALIASES: Record<string, string> = { h264: 'avc1', hevc: 'hev1' };
  const supportedVideoCodecs = ['h264', 'hevc', 'av1', 'vp8', 'vp9'].filter((codec) =>
    canVideo(`video/mp4; codecs="${CODEC_ALIASES[codec] ?? codec}"`),
  );
  const supportedAudioCodecs = ['mp3', 'aac', 'vorbis', 'opus', 'wav', 'flac'].filter((codec) =>
    canAudio(codec === 'wav' ? 'audio/wav' : `audio/${codec}`),
  );
  const clientName = `Koko Web (${navigator.userAgent.split(' ').at(-1)})`;
  return {
    client_type: 'web',
    client_name: clientName,
    supported_containers: ['mp4', 'webm', 'mkv', 'mp3', 'flac', 'wav', 'ogg'],
    supported_video_codecs: supportedVideoCodecs,
    supported_audio_codecs: supportedAudioCodecs,
    supported_subtitle_formats: ['vtt'],
    max_video_width: 0,
    max_video_height: 0,
    max_bitrate_kbps: 0,
    supports_adaptive_streaming: false,
    prefer_hls: false,
  };
}
