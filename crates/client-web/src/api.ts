import {
  addMockLibrary,
  createMockUser,
  deleteMockMissingItems,
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
  getMockPerson,
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
  clearMockMetadataCache,
  runMockScheduledTask,
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

export interface ProfileImageUploadRequest {
  mime_type: string;
  data_base64: string;
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
  season_number?: number | null;
  episode_number?: number | null;
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
  genres: string[];
  release_year?: number;
  logo_url?: string;
  rating?: number;
  content_rating?: string;
  linked_media_type?: string;
  trailer_title?: string;
  trailer_url?: string;
  extras: MediaItemExtra[];
  artwork_updated_at?: number;
  playback_position_ms?: number;
  playback_duration_ms?: number;
  playback_completed?: boolean;
  watch_count?: number;
  last_watched_at?: number | null;
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
  cached_backdrop_path?: string;
  refresh_state?: string;
  last_refreshed_at?: number;
  next_refresh_at?: number;
  refresh_error?: string;
  updated_at?: number;
}

export interface MediaAudioTrack {
  index: number;
  label: string;
  codec?: string;
  language?: string;
  default: boolean;
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
  theme_song_url?: string;
  item_ids: number[];
  item_count: number;
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

export function getWebClientProfile(): ClientProfile {
  const videoProbe = document.createElement('video');
  const audioProbe = document.createElement('audio');
  const supportsVideoType = (type: string): boolean => videoProbe.canPlayType(type) === 'probably';
  const supportsAudioType = (type: string): boolean => audioProbe.canPlayType(type) !== '';
  const supportedContainers = new Set<string>();
  const supportedVideoCodecs = new Set<string>();
  const supportedAudioCodecs = new Set<string>();

  if (supportsVideoType('video/mp4; codecs="avc1.42E01E, mp4a.40.2"')) {
    supportedContainers.add('mp4');
    supportedContainers.add('m4v');
    supportedVideoCodecs.add('h264');
    supportedAudioCodecs.add('aac');
  }
  if (supportsVideoType('video/mp4; codecs="hvc1.1.6.L93.B0, mp4a.40.2"')) {
    supportedContainers.add('mp4');
    supportedContainers.add('m4v');
    supportedVideoCodecs.add('hevc');
    supportedAudioCodecs.add('aac');
  }
  if (supportsVideoType('video/mp4; codecs="av01.0.05M.08, mp4a.40.2"')) {
    supportedContainers.add('mp4');
    supportedVideoCodecs.add('av1');
    supportedAudioCodecs.add('aac');
  }
  if (supportsVideoType('video/webm; codecs="vp8, vorbis"')) {
    supportedContainers.add('webm');
    supportedVideoCodecs.add('vp8');
    supportedAudioCodecs.add('vorbis');
  }
  if (supportsVideoType('video/webm; codecs="vp9, opus"')) {
    supportedContainers.add('webm');
    supportedVideoCodecs.add('vp9');
    supportedAudioCodecs.add('opus');
  }
  if (supportsVideoType('video/webm; codecs="av01.0.05M.08, opus"')) {
    supportedContainers.add('webm');
    supportedVideoCodecs.add('av1');
    supportedAudioCodecs.add('opus');
  }
  if (supportsAudioType('audio/mpeg')) {
    supportedContainers.add('mp3');
    supportedAudioCodecs.add('mp3');
  }
  if (supportsAudioType('audio/mp4; codecs="mp4a.40.2"')) {
    supportedContainers.add('m4a');
    supportedAudioCodecs.add('aac');
  }
  if (supportsAudioType('audio/ogg; codecs="vorbis"')) {
    supportedContainers.add('ogg');
    supportedAudioCodecs.add('vorbis');
  }
  if (supportsAudioType('audio/ogg; codecs="opus"')) {
    supportedContainers.add('ogg');
    supportedAudioCodecs.add('opus');
  }
  if (supportsAudioType('audio/wav')) {
    supportedContainers.add('wav');
  }
  if (supportsAudioType('audio/flac')) {
    supportedContainers.add('flac');
    supportedAudioCodecs.add('flac');
  }

  return {
    client_type: 'web',
    client_name: `Koko Web (${navigator.userAgent.split(' ').pop()})`,
    supported_containers: [...supportedContainers],
    supported_video_codecs: [...supportedVideoCodecs],
    supported_audio_codecs: [...supportedAudioCodecs],
    supported_subtitle_formats: ['vtt'],
    max_video_width: 0,
    max_video_height: 0,
    max_bitrate_kbps: 0,
    supports_adaptive_streaming: false,
    prefer_hls: false,
  };
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

export interface CreateSessionRequest {
  item_id: number;
  client_profile: ClientProfile;
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
  scanner: string;
  metadata_providers: string[];
  metadata_language_mode: 'auto' | 'manual';
  metadata_languages: string[];
  allowed_user_ids: number[];
}

export interface SettingsSnapshot {
  general: {
    data_dir: string;
  };
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
    window: {
      start_time: string;
      stop_time: string;
      weekdays: ScheduledTaskWeekday[];
    };
    metadata_refresh: {
      enabled: boolean;
    };
    trash_cleanup: {
      enabled: boolean;
      missing_item_auto_delete_days?: number | null;
      interval_days: number;
    };
    database_maintenance: {
      enabled: boolean;
      interval_days: number;
    };
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

export type ScheduledTaskWeekday =
  | 'monday'
  | 'tuesday'
  | 'wednesday'
  | 'thursday'
  | 'friday'
  | 'saturday'
  | 'sunday';

export type ScheduledTaskId = 'metadata_refresh' | 'trash_cleanup' | 'database_maintenance';

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
        return searchMockItems(query) as T;
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

        const personMatch = url.pathname.match(/^\/api\/v1\/people\/(\d+)$/);
        if (personMatch) {
          return getMockPerson(Number(personMatch[1])) as T;
        }

        const itemMatch = url.pathname.match(/^\/api\/v1\/items\/(\d+)$/);
        if (itemMatch) {
          const item = getMockItem(Number(itemMatch[1]));
          if (!item) {
            throw new Error('404 Not Found');
          }

          return item as T;
        }

        const sessionStreamMatch = url.pathname.match(/^\/api\/v1\/sessions\/([^/]+)\/stream$/);
        if (sessionStreamMatch) {
          throw new Error('501 Not Implemented (mock streaming not fully supported)');
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

  if (method === 'POST' && url.pathname === '/api/v1/settings/metadata-cache/clear') {
    return clearMockMetadataCache() as T;
  }

  const scheduledTaskRunMatch = url.pathname.match(/^\/api\/v1\/scheduled-tasks\/([^/]+)\/run$/);
  if (method === 'POST' && scheduledTaskRunMatch) {
    return runMockScheduledTask(scheduledTaskRunMatch[1] as ScheduledTaskId) as T;
  }

  const removeLibraryMatch = url.pathname.match(/^\/api\/v1\/settings\/libraries\/(\d+)$/);
  if (method === 'DELETE' && removeLibraryMatch) {
    return removeMockLibrary(Number(removeLibraryMatch[1])) as T;
  }

  const missingItemsMatch = url.pathname.match(/^\/api\/v1\/libraries\/(\d+)\/missing$/);
  if (method === 'DELETE' && missingItemsMatch) {
    return deleteMockMissingItems(Number(missingItemsMatch[1])) as T;
  }

  const deleteSessionMatch = url.pathname.match(/^\/api\/v1\/sessions\/([^/]+)$/);
  if (method === 'DELETE' && deleteSessionMatch) {
    return undefined as T;
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

  if (method === 'POST' && url.pathname === '/api/v1/sessions') {
    // Basic mock for create_session
    const request = body as CreateSessionRequest;
    const item = getMockItem(request.item_id);
    const preferredLanguages = getMockBootstrap().current_user?.preferred_metadata_languages ?? ['en-US'];
    const audioStreamIndex = item?.audio_tracks?.find((track) => {
      const language = track.language?.toLowerCase();
      return language && preferredLanguages.some((preferred) => {
        const normalized = preferred.toLowerCase();
        return normalized.startsWith(language) || language.startsWith(normalized.split('-')[0]);
      });
    })?.index;
    return {
      session_id: 'mock-session-123',
      item_id: request.item_id,
      client_profile: request.client_profile,
      decision: getMockPlayback(request.item_id),
      created_at: Date.now(),
      audio_stream_index: audioStreamIndex,
    } as T;
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

export async function searchItems(query: string): Promise<MediaSearchResult[]> {
  const params = new URLSearchParams({ query });

  const results = await requestJson<MediaSearchResult[]>('GET', `/api/v1/search?${params.toString()}`);
  const normalizedQuery = query.trim().toLowerCase();
  if (!normalizedQuery || !'playlists'.includes(normalizedQuery)) {
    return results;
  }

  return [
    ...results,
    {
      result_type: 'playlist',
      playlist: {
        id: 'Playlists',
        name: 'Playlists',
        overview: 'Playlist creation is planned. Items will appear here when playlists are available.',
        item_count: 0,
      },
    },
  ];
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

export function getPerson(personId: number): Promise<MetadataPersonResponse> {
  return requestJson<MetadataPersonResponse>('GET', `/api/v1/people/${personId}`);
}

export function getPersonImageUrl(personId: number): string {
  return `${getStoredApiBase()}/api/v1/people/${personId}/image`;
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

export function deleteMissingItems(libraryId: number): Promise<MissingItemsCleanupResponse> {
  return requestJson<MissingItemsCleanupResponse>('DELETE', `/api/v1/libraries/${libraryId}/missing`);
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

export function clearMetadataCache(): Promise<MetadataCacheClearResponse> {
  return requestJson<MetadataCacheClearResponse>('POST', '/api/v1/settings/metadata-cache/clear');
}

export function runScheduledTask(taskId: ScheduledTaskId): Promise<ScheduledTaskRunResponse> {
  return requestJson<ScheduledTaskRunResponse>('POST', `/api/v1/scheduled-tasks/${taskId}/run`);
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

export function getSessionStreamUrl(sessionId: string, startMs?: number, audioStreamIndex?: number): string {
  const params = new URLSearchParams();
  if (typeof startMs === 'number' && Number.isFinite(startMs) && startMs > 0) {
    params.set('start_ms', String(Math.max(0, Math.floor(startMs))));
  }
  if (typeof audioStreamIndex === 'number' && Number.isFinite(audioStreamIndex) && audioStreamIndex >= 0) {
    params.set('audio_stream_index', String(Math.floor(audioStreamIndex)));
  }
  const suffix = params.toString() ? `?${params.toString()}` : '';
  return `${getStoredApiBase()}/api/v1/sessions/${sessionId}/stream${suffix}`;
}

export function createPlaybackSession(request: CreateSessionRequest): Promise<PlaybackSession> {
  return requestJson<PlaybackSession>('POST', '/api/v1/sessions', request);
}

export function deletePlaybackSession(sessionId: string): Promise<void> {
  return requestJson<void>('DELETE', `/api/v1/sessions/${sessionId}`);
}

export function getArtworkUrl(itemId: number, kind: 'poster' | 'backdrop' | 'logo' = 'poster', revision?: number): string {
  const params = new URLSearchParams({ kind });
  if (typeof revision === 'number') {
    params.set('rev', String(revision));
  }

  return `${getStoredApiBase()}/api/v1/items/${itemId}/artwork?${params.toString()}`;
}
