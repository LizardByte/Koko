// Data layer for the Svelte PoC.
//
// Mirrors the vanilla client's contract (../client-web/src/api.ts) for the
// catalog/auth/home/logs endpoints. For a full migration the vanilla api.ts
// + mockApi.ts would be copied wholesale; here we port the subset the PoC's
// views actually use, with the same types and the same VITE_USE_MOCK_API
// toggle so the dev workflow carries over verbatim.

// ---------------------------------------------------------------------------
// Types (copied from ../client-web/src/api.ts)
// ---------------------------------------------------------------------------

export interface ServerCapabilities {
  app_name: string;
  version: string;
  server_url: string;
  https_enabled: boolean;
  libraries_configured: number;
  api_versions: string[];
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

export interface MediaLibrary {
  id: number;
  name: string;
  path: string;
  kind: string;
  status: string;
  total_files: number;
  video_files: number;
  audio_files: number;
  image_files: number;
  book_files: number;
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
  duration_ms?: number;
  genres: string[];
  overview?: string;
  backdrop_url?: string;
  logo_url?: string;
  has_metadata?: boolean;
  artwork_updated_at?: number;
  playback_position_ms?: number;
  playback_duration_ms?: number;
  playback_completed?: boolean;
  watch_count?: number;
}

export interface MediaItemDetail extends MediaItemSummary {
  file_size?: number;
  container?: string;
  bit_rate?: number;
  video_codec?: string;
  audio_codec?: string;
  poster_url?: string;
  backdrop_url?: string;
  tagline?: string;
  overview?: string;
  release_year?: number;
  logo_url?: string;
  rating?: number;
  content_rating?: string;
  extras: MediaItemExtra[];
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
  item_ids: number[];
  item_count: number;
}

export interface MediaHome {
  library_id?: number;
  shelves: MediaShelf[];
  collections: MediaCollectionSummary[];
}

export interface MetadataPersonSummary {
  id: number;
  provider_id: string;
  external_id?: string;
  locale_key: string;
  name: string;
  known_for: string[];
  biography?: string;
  image_url?: string;
  updated_at?: number;
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

export const AUTH_TOKEN_STORAGE_KEY = 'koko-client-web-auth-token';

// Same toggle as the vanilla client. `vite dev --mode mock` loads .env.mock.
const USE_MOCK_API = import.meta.env.VITE_USE_MOCK_API === 'true';

export function isMockApi(): boolean {
  return USE_MOCK_API;
}

function resolveApiBase(): string {
  const fromEnv = (import.meta.env.VITE_API_BASE_URL as string | undefined)?.trim();
  if (fromEnv) {
    return fromEnv;
  }
  return globalThis.location.origin;
}

export function getStoredAuthToken(): string | null {
  return globalThis.localStorage.getItem(AUTH_TOKEN_STORAGE_KEY);
}

export function setStoredAuthToken(token: string): void {
  globalThis.localStorage.setItem(AUTH_TOKEN_STORAGE_KEY, token);
}

export function clearStoredAuthToken(): void {
  globalThis.localStorage.removeItem(AUTH_TOKEN_STORAGE_KEY);
}

async function requestJson<T>(method: string, path: string, body?: unknown): Promise<T> {
  const headers: Record<string, string> = {};
  if (body !== undefined) {
    headers['Content-Type'] = 'application/json';
  }
  const token = getStoredAuthToken();
  if (token) {
    headers.Authorization = `Bearer ${token}`;
  }
  const response = await fetch(resolveApiBase() + path, {
    method,
    headers,
    body: body !== undefined ? JSON.stringify(body) : undefined,
  });
  if (!response.ok) {
    throw new Error(`${method} ${path} failed: ${response.status} ${response.statusText}`);
  }
  if (response.status === 204) {
    return undefined as T;
  }
  return (await response.json()) as T;
}

// ---------------------------------------------------------------------------
// URL helpers (mirror ../client-web/src/api.ts)
// ---------------------------------------------------------------------------

export function resolveApiUrl(path: string): string {
  if (/^https?:\/\//i.test(path)) {
    return path;
  }
  const normalized = path.startsWith('/') ? path : `/${path}`;
  return `${resolveApiBase()}${normalized}`;
}

export function getArtworkUrl(
  itemId: number,
  kind: 'poster' | 'backdrop' | 'logo' = 'poster',
  revision?: number,
): string {
  if (USE_MOCK_API) {
    // The mock has no real artwork endpoint; return a deterministic placeholder
    // so cards render with a visible gradient + title instead of a broken img.
    return `mock://artwork/${itemId}/${kind}`;
  }
  const params = new URLSearchParams({ kind });
  if (typeof revision === 'number') {
    params.set('rev', String(revision));
  }
  return `${resolveApiBase()}/api/v1/items/${itemId}/artwork?${params.toString()}`;
}

// ---------------------------------------------------------------------------
// API surface
// ---------------------------------------------------------------------------

export function getAppBootstrap(): Promise<AppBootstrapResponse> {
  if (USE_MOCK_API) {
    return Promise.resolve(getMockBootstrap());
  }
  return requestJson<AppBootstrapResponse>('GET', '/api/v1/bootstrap');
}

export function loginUser(request: LoginRequest): Promise<TokenResponse> {
  if (USE_MOCK_API) {
    return loginMockUser(request);
  }
  return requestJson<TokenResponse>('POST', '/login', request);
}

export function getLibraries(): Promise<MediaLibrary[]> {
  if (USE_MOCK_API) {
    return Promise.resolve(getMockLibraries());
  }
  return requestJson<MediaLibrary[]>('GET', '/api/v1/libraries');
}

export function getHome(libraryId?: number): Promise<MediaHome> {
  if (USE_MOCK_API) {
    return Promise.resolve(getMockHome(libraryId));
  }
  const suffix = typeof libraryId === 'number' ? `?library_id=${libraryId}` : '';
  return requestJson<MediaHome>('GET', `/api/v1/home${suffix}`);
}

export function getItem(itemId: number): Promise<MediaItemDetail> {
  if (USE_MOCK_API) {
    return Promise.resolve(getMockItem(itemId));
  }
  return requestJson<MediaItemDetail>('GET', `/api/v1/items/${itemId}`);
}

export function getPerson(personId: number): Promise<{ person: MetadataPersonSummary }> {
  if (USE_MOCK_API) {
    return Promise.resolve({ person: getMockPerson(personId) });
  }
  return requestJson<{ person: MetadataPersonSummary }>('GET', `/api/v1/people/${personId}`);
}

export async function getLogs(filters?: {
  level?: string;
  module?: string;
  search?: string;
  since?: string;
  until?: string;
  limit?: number;
}): Promise<LogEntriesResponse> {
  if (USE_MOCK_API) {
    return getMockLogs(
      filters?.level,
      filters?.module,
      filters?.search,
      filters?.since,
      filters?.until,
      filters?.limit ?? 200,
    );
  }
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

// ---------------------------------------------------------------------------
// Mock implementation (subset of ../client-web/src/mockApi.ts)
// ---------------------------------------------------------------------------

const MOCK_LIBRARIES: MediaLibrary[] = [
  {
    id: 1,
    name: 'Movies',
    path: '/media/movies',
    kind: 'movie',
    status: 'ready',
    total_files: 1284,
    video_files: 842,
    audio_files: 0,
    image_files: 442,
    book_files: 0,
  },
  {
    id: 2,
    name: 'TV Shows',
    path: '/media/tv',
    kind: 'show',
    status: 'ready',
    total_files: 9821,
    video_files: 3120,
    audio_files: 0,
    image_files: 6701,
    book_files: 0,
  },
  {
    id: 3,
    name: 'Music',
    path: '/media/music',
    kind: 'album',
    status: 'scanning',
    total_files: 21450,
    video_files: 0,
    audio_files: 19840,
    image_files: 1610,
    book_files: 0,
  },
];

function mockItem(
  id: number,
  libraryId: number,
  itemType: string,
  title: string,
  partial: Partial<MediaItemSummary> = {},
): MediaItemSummary {
  return {
    id,
    library_id: libraryId,
    item_type: itemType,
    display_title: title,
    relative_path: `${itemType}-${id}`,
    media_kind: itemType === 'movie' || itemType === 'episode' ? 'video' : itemType === 'album' ? 'audio' : 'container',
    playable: itemType === 'movie' || itemType === 'episode' || itemType === 'track',
    child_count: 0,
    genres: [],
    has_metadata: true,
    ...partial,
  };
}

const MOCK_ITEMS: MediaItemSummary[] = [
  mockItem(101, 1, 'movie', 'The Phantom Stellar', {
    display_subtitle: '2024',
    genres: ['Science Fiction', 'Adventure'],
    overview: 'A lone pilot navigates a dying star system in search of a habitable world.',
    backdrop_url: '',
    duration_ms: 7_284_000,
  }),
  mockItem(102, 1, 'movie', 'Coastline Confidential', {
    display_subtitle: '2023',
    genres: ['Thriller', 'Mystery'],
    overview: 'A retired inspector is pulled back for one last case on the rainy coast.',
    duration_ms: 6_412_000,
  }),
  mockItem(103, 1, 'movie', 'The Lantern Keeper', {
    display_subtitle: '2022',
    genres: ['Drama'],
    overview: 'On a remote island, a keeper tends a light that must never go out.',
    duration_ms: 5_980_000,
  }),
  mockItem(104, 1, 'movie', 'Neon Vermilion', {
    display_subtitle: '2025',
    genres: ['Action', 'Cyberpunk'],
    overview: 'In a rain-soaked megacity, a courier carries a package everyone wants.',
    duration_ms: 7_010_000,
  }),
  mockItem(201, 2, 'show', 'Mock Show', {
    display_subtitle: '5 seasons',
    child_count: 5,
    genres: ['Comedy', 'Drama'],
    overview: 'The everyday chaos of a family running a small-town diner.',
    available_season_count: 5,
  }),
  mockItem(202, 2, 'show', 'The Silent Frequency', {
    display_subtitle: '3 seasons',
    child_count: 3,
    genres: ['Mystery', 'Science Fiction'],
    overview: 'A radio operator picks up a signal that should not exist.',
    available_season_count: 3,
  }),
  mockItem(203, 2, 'show', 'Paper Cities', {
    display_subtitle: '2 seasons',
    child_count: 2,
    genres: ['Drama'],
    overview: 'An architect rediscovers the towns she once designed.',
    available_season_count: 2,
  }),
  mockItem(301, 3, 'album', 'Midnight Cartography', {
    display_subtitle: 'Vela Sound',
    child_count: 11,
    genres: ['Electronic'],
    overview: 'Ambient electronic mapping the terrain of sleepless nights.',
  }),
  mockItem(302, 3, 'album', 'Tin Roof Hymns', {
    display_subtitle: 'The Saltlick Choir',
    child_count: 9,
    genres: ['Folk'],
    overview: 'Acoustic folk recorded in a single barn session.',
  }),
];

export function getMockLibraries(): MediaLibrary[] {
  return structuredClone(MOCK_LIBRARIES);
}

export function getMockBootstrap(): AppBootstrapResponse {
  return {
    has_users: true,
    current_user: {
      id: 1,
      username: 'admin',
      admin: true,
      preferred_metadata_languages: ['en'],
      profile_image_url: '',
    },
  };
}

export function loginMockUser(request: LoginRequest): Promise<TokenResponse> {
  // Mock credentials match the vanilla client's seeded mock user.
  if (request.username === 'admin' && request.password === 'adminpass') {
    return Promise.resolve({ token: 'mock-jwt-token-admin' });
  }
  return Promise.reject(new Error('Invalid username or password'));
}

export function getMockHome(_libraryId?: number): MediaHome {
  const movies = MOCK_ITEMS.filter((i) => i.library_id === 1);
  const shows = MOCK_ITEMS.filter((i) => i.library_id === 2);
  const albums = MOCK_ITEMS.filter((i) => i.library_id === 3);
  return {
    shelves: [
      { id: 'continue', title: 'Continue Watching', items: [movies[0], movies[3]] },
      { id: 'recent-movies', title: 'Recently Added Movies', items: movies },
      { id: 'recent-shows', title: 'Popular Shows', items: shows },
      { id: 'recent-albums', title: 'New Music', items: albums },
    ],
    collections: [
      {
        id: 'col-action',
        provider_id: 'mock',
        external_id: 'action-night',
        name: 'Action Night',
        overview: 'Explosions, chases, and high stakes.',
        item_ids: [104, 101],
        item_count: 2,
      },
    ],
  };
}

export function getMockItem(itemId: number): MediaItemDetail {
  const summary = MOCK_ITEMS.find((i) => i.id === itemId) ?? MOCK_ITEMS[0];
  return {
    ...summary,
    tagline: summary.genres[0] ? `A ${summary.genres[0].toLowerCase()} story.` : undefined,
    poster_url: '',
    backdrop_url: '',
    overview: summary.overview ?? 'No overview available.',
    release_year: summary.display_subtitle ? Number(summary.display_subtitle) : undefined,
    rating: 7.4 + (itemId % 3) * 0.6,
    content_rating: summary.item_type === 'movie' ? 'PG-13' : 'TV-14',
    audio_tracks: [
      { index: 0, label: 'English (5.1)', codec: 'aac', language: 'en', default: true },
    ],
    subtitle_tracks: [
      { index: 0, label: 'English', format: 'vtt', url: `mock://subtitles/${itemId}/en.vtt` },
    ],
    extras: [],
    hierarchy: [],
    children:
      summary.item_type === 'show'
        ? Array.from({ length: summary.available_season_count ?? 1 }, (_, idx) =>
            mockItem(itemId * 100 + idx, summary.library_id, 'season', `Season ${idx + 1}`, {
              parent_id: itemId,
              display_subtitle: `${idx + 1} season`,
              child_count: 10,
            }),
          )
        : [],
  };
}

export function getMockPerson(personId: number): MetadataPersonSummary {
  return {
    id: personId,
    provider_id: 'mock',
    locale_key: 'en',
    name: personId === 1 ? 'Alex Rivera' : 'Jordan Hale',
    known_for: ['The Phantom Stellar', 'Coastline Confidential'],
    biography:
      'A versatile performer whose work spans independent cinema and large-scale productions.',
    image_url: '',
    updated_at: Date.now(),
  };
}

export function getMockLogs(
  level?: string,
  moduleFilter?: string,
  search?: string,
  since?: string,
  until?: string,
  limit = 200,
): LogEntriesResponse {
  const sinceTime = since ? new Date(since).getTime() : Number.NaN;
  const untilTime = until ? new Date(until).getTime() : Number.NaN;
  const entries: LogEntry[] = [
    {
      timestamp: '2026-04-22T09:12:35.853-04:00',
      level: 'INFO',
      module: 'koko::web::routes::media',
      source_file_path: 'src/web/routes/media.rs',
      line_number: 540,
      message:
        'Completed TMDB metadata refresh for media item 201 "Mock Show" (show) in library 2 [Mock Show]',
    },
    {
      timestamp: '2026-04-22T09:12:00.810-04:00',
      level: 'WARN',
      module: 'koko::web::routes::media',
      source_file_path: 'src/web/routes/media.rs',
      line_number: 589,
      message:
        'Failed to fetch refreshed TMDB metadata snapshot for media item 417 "Season 1" (season) in library 2 [The Simpsons/Season 1] using target tv:456:season:1 (tv_season): TMDB season lookup failed with status 404 Not Found',
    },
    {
      timestamp: '2026-04-22T09:10:49.079-04:00',
      level: 'DEBUG',
      module: 'reqwest::connect',
      source_file_path: 'src/connect.rs',
      line_number: 118,
      message: 'starting new connection: https://api.themoviedb.org/',
    },
  ].filter((entry) => {
    const levelMatches = level ? entry.level.toLowerCase() === level.toLowerCase() : true;
    const moduleMatches = moduleFilter
      ? entry.module.toLowerCase().includes(moduleFilter.toLowerCase())
      : true;
    const searchMatches = search
      ? `${entry.message} ${entry.module} ${entry.source_file_path}`
          .toLowerCase()
          .includes(search.toLowerCase())
      : true;
    const timestamp = new Date(entry.timestamp).getTime();
    const sinceMatches = Number.isNaN(sinceTime) || timestamp >= sinceTime;
    const untilMatches = Number.isNaN(untilTime) || timestamp <= untilTime;
    return levelMatches && moduleMatches && searchMatches && sinceMatches && untilMatches;
  });

  return {
    log_path: 'C:/Users/Mock/AppData/Local/Koko/data/koko.log',
    entries: entries.slice(0, Math.max(1, limit)),
  };
}
