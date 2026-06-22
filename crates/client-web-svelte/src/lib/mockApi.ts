// Mock API layer — full-fidelity port of ../client-web/src/mockApi.ts.
//
// Contains the verbatim seed data (Matrix / Game of Thrones themed) and the
// stateful behaviors of the vanilla mock. Four behavioral quirks are
// reproduced deliberately (see dispatch() for the dev-mode fallback):
//   1. applyMockPlaybackProgress overlays position/completed only when a
//      mock-token-<id> is the active auth token.
//   2. getMockLibraries recomputes metadata refresh counters on every read.
//   3. getMockItems strips 7 detail-only fields from MediaItemDetail.
//   4. refreshItemMetadata/refreshLibraryMetadata use setTimeout (900/1200ms)
//      to flip pending -> fresh, driving transient system activities.
//
// The dev-mode silent-fallback-to-mock lives in ./api.ts (requestJson).

import type {
  AppBootstrapResponse,
  BootstrapUser,
  CreateSessionRequest,
  ItemMetadataMatch,
  ItemMetadataResponse,
  LinkMetadataRequest,
  LogEntriesResponse,
  MediaCollectionSummary,
  MediaHome,
  MediaItemDetail,
  MediaItemSummary,
  MediaLibrary,
  MediaLibrarySettings,
  MediaSearchResult,
  MetadataCacheClearResponse,
  MetadataPersonItemCredit,
  MetadataPersonResponse,
  MetadataProviderStatus,
  MetadataSearchResult,
  MetadataSearchOptions,
  MissingItemsCleanupResponse,
  PlaybackDecision,
  PlaybackProgressRequest,
  PlaybackSession,
  ScheduledTaskId,
  ScheduledTaskRunResponse,
  ServerCapabilities,
  SettingsResponse,
  SettingsSnapshot,
  SystemActivitiesResponse,
  TokenResponse,
  UpdateUserRequest,
  CreateUserRequest,
} from './api';

// ---------------------------------------------------------------------------
// Internal types
// ---------------------------------------------------------------------------

interface MockUserRecord extends BootstrapUser {
  password: string;
  pin?: string;
}

interface MockPlaybackProgress extends PlaybackProgressRequest {
  watch_count: number;
  last_watched_at?: number;
}

// ---------------------------------------------------------------------------
// Seed data — VERBATIM from vanilla mockApi.ts
// ---------------------------------------------------------------------------

const libraries: MediaLibrary[] = [
  {
    id: 1,
    name: 'Movies',
    path: 'C:/Media/Movies',
    paths: ['C:/Media/Movies', 'D:/Overflow/Movies'],
    recursive: true,
    kind: 'movies',
    scanner: 'movies',
    metadata_providers: ['tmdb'],
    metadata_language_mode: 'auto',
    metadata_languages: ['en-US'],
    status: 'available',
    scan_revision: 6,
    last_scanned_at: 1760923200,
    total_files: 2,
    video_files: 2,
    audio_files: 0,
    image_files: 0,
    book_files: 0,
    other_files: 0,
    metadata_refresh_total: 0,
    metadata_refresh_pending: 0,
    metadata_refresh_completed: 0,
    metadata_refresh_failed: 0,
    missing_files: 0,
    missing_items: 0,
  },
  {
    id: 2,
    name: 'Shows',
    path: 'C:/Media/Shows',
    paths: ['C:/Media/Shows'],
    recursive: true,
    kind: 'shows',
    scanner: 'shows',
    metadata_providers: ['tmdb'],
    metadata_language_mode: 'auto',
    metadata_languages: ['en-US', 'ja-JP'],
    status: 'available',
    scan_revision: 5,
    last_scanned_at: 1760923150,
    total_files: 1,
    video_files: 1,
    audio_files: 0,
    image_files: 0,
    book_files: 0,
    other_files: 0,
    metadata_refresh_total: 0,
    metadata_refresh_pending: 0,
    metadata_refresh_completed: 0,
    metadata_refresh_failed: 0,
    missing_files: 0,
    missing_items: 0,
  },
  {
    id: 3,
    name: 'Music',
    path: 'C:/Media/Music',
    paths: ['C:/Media/Music'],
    recursive: true,
    kind: 'music',
    scanner: 'music',
    metadata_providers: [],
    metadata_language_mode: 'auto',
    metadata_languages: ['en-US'],
    status: 'available',
    scan_revision: 4,
    last_scanned_at: 1760923100,
    total_files: 2,
    video_files: 0,
    audio_files: 2,
    image_files: 0,
    book_files: 0,
    other_files: 0,
    metadata_refresh_total: 0,
    metadata_refresh_pending: 0,
    metadata_refresh_completed: 0,
    metadata_refresh_failed: 0,
    missing_files: 0,
    missing_items: 0,
  },
];

// Helper to build the summary projection of an item (what getMockItems returns
// — strips the detail-only fields per vanilla getMockItems lines 1176-1180).
const DETAIL_ONLY_FIELDS = [
  'file_size',
  'container',
  'bit_rate',
  'video_codec',
  'audio_codec',
  'metadata_json',
  'metadata_updated_at',
] as const;

function summarizeItem(detail: MediaItemDetail): MediaItemSummary {
  const clone = structuredClone(detail) as unknown as Record<string, unknown>;
  for (const field of DETAIL_ONLY_FIELDS) {
    delete clone[field];
  }
  return clone as unknown as MediaItemSummary;
}

// Summary views of the show/season/episode chain, used as nested
// hierarchy/children references. Built from the detail entries below.
const SHOW_201_SUMMARY: MediaItemSummary = {
  id: 201,
  library_id: 2,
  item_type: 'show',
  display_title: 'Mock Show',
  relative_path: 'Mock Show',
  media_kind: 'video',
  playable: false,
  child_count: 1,
  duration_ms: 2_700_000,
  modified_at: 1760923150,
  genres: ['Drama', 'Fantasy'],
  has_metadata: true,
  metadata_refresh_state: 'fresh',
};

const SEASON_202_SUMMARY: MediaItemSummary = {
  id: 202,
  library_id: 2,
  parent_id: 201,
  item_type: 'season',
  display_title: 'Season 1',
  relative_path: 'Mock Show/Season 1',
  media_kind: 'video',
  playable: false,
  child_count: 1,
  season_number: 1,
  duration_ms: 2_700_000,
  modified_at: 1760923150,
  genres: ['Drama', 'Fantasy'],
  has_metadata: true,
  metadata_refresh_state: 'fresh',
};

const EPISODE_203_SUMMARY: MediaItemSummary = {
  id: 203,
  library_id: 2,
  parent_id: 202,
  item_type: 'episode',
  display_title: 'Mock Episode',
  relative_path: 'Mock Show/Season 1/episode-01.mp4',
  media_kind: 'video',
  playable: true,
  child_count: 0,
  season_number: 1,
  episode_number: 1,
  duration_ms: 2_700_000,
  width: 1280,
  height: 720,
  modified_at: 1760923100,
  genres: ['Drama', 'Fantasy'],
  has_metadata: true,
  metadata_refresh_state: 'fresh',
};

const items: MediaItemDetail[] = [
  {
    id: 101,
    library_id: 1,
    item_type: 'movie',
    display_title: 'Mock Movie',
    relative_path: 'Action/mock-movie.mp4',
    media_kind: 'video',
    playable: true,
    child_count: 0,
    duration_ms: 5_400_000,
    width: 1920,
    height: 1080,
    modified_at: 1760923200,
    file_size: 1_610_612_736,
    container: 'mp4',
    bit_rate: 2_400_000,
    video_codec: 'h264',
    audio_codec: 'aac',
    metadata_json: JSON.stringify(
      {
        format: { format_name: 'mp4', duration: '5400.0' },
        streams: [
          { codec_type: 'video', codec_name: 'h264' },
          {
            codec_type: 'audio',
            codec_name: 'aac',
            tags: { language: 'jpn', title: 'Japanese' },
            disposition: { default: 1 },
          },
          {
            codec_type: 'audio',
            codec_name: 'aac',
            tags: { language: 'eng', title: 'English' },
            disposition: { default: 0 },
          },
        ],
      },
      null,
      2,
    ),
    metadata_updated_at: 1760923200,
    poster_url: '/api/v1/items/101/artwork?kind=poster',
    backdrop_url: '/api/v1/items/101/artwork?kind=backdrop',
    theme_song_url: '/api/v1/items/101/theme',
    tagline: 'Welcome to the real world.',
    overview:
      'A computer hacker learns the true nature of reality and his role in the war against its controllers.',
    genres: ['Action', 'Science Fiction'],
    release_year: 1999,
    linked_media_type: 'movie',
    has_metadata: true,
    metadata_refresh_state: 'fresh',
    artwork_updated_at: 1760923200,
    trailer_title: 'Official Trailer',
    trailer_url: 'https://www.youtube.com/watch?v=vKQi3bBA1y8',
    extras: [
      {
        extra_type: 'trailer',
        title: 'Official Trailer',
        url: 'https://www.youtube.com/watch?v=vKQi3bBA1y8',
        duration_seconds: 136,
        thumbnail_url: 'https://i.ytimg.com/vi/vKQi3bBA1y8/hqdefault.jpg',
      },
      {
        extra_type: 'theme_song',
        title: 'Main Theme',
        url: 'https://www.youtube.com/watch?v=SLBACEP6LsI',
        duration_seconds: 74,
        thumbnail_url: 'https://i.ytimg.com/vi/SLBACEP6LsI/hqdefault.jpg',
      },
    ],
    audio_tracks: [
      { index: 0, label: 'Japanese', codec: 'aac', language: 'jpn', default: true },
      { index: 1, label: 'English', codec: 'aac', language: 'eng', default: false },
    ],
    subtitle_tracks: [{ index: 0, label: 'EN', format: 'SRT', url: '/api/v1/items/101/subtitles/0' }],
    hierarchy: [],
    children: [],
  },
  {
    id: 201,
    library_id: 2,
    item_type: 'show',
    display_title: 'Mock Show',
    relative_path: 'Mock Show',
    media_kind: 'video',
    playable: false,
    child_count: 1,
    duration_ms: 2_700_000,
    modified_at: 1760923150,
    genres: ['Drama', 'Fantasy'],
    linked_media_type: 'tv',
    has_metadata: true,
    metadata_refresh_state: 'fresh',
    theme_song_url: 'https://www.youtube.com/watch?v=uXZd_W5B7N0',
    extras: [],
    audio_tracks: [],
    subtitle_tracks: [],
    playback_target: {
      item_id: 203,
      start_ms: 74_000,
      label: 'Resume S01E01',
      display_title: 'Mock Episode',
      season_number: 1,
      episode_number: 1,
      resume: true,
    },
    restart_playback_target: {
      item_id: 203,
      start_ms: 0,
      label: 'Start show',
      display_title: 'Mock Episode',
      season_number: 1,
      episode_number: 1,
      resume: false,
    },
    hierarchy: [],
    children: [{ ...SEASON_202_SUMMARY }],
  },
  {
    id: 202,
    library_id: 2,
    parent_id: 201,
    item_type: 'season',
    display_title: 'Season 1',
    relative_path: 'Mock Show/Season 1',
    media_kind: 'video',
    playable: false,
    child_count: 1,
    season_number: 1,
    duration_ms: 2_700_000,
    modified_at: 1760923150,
    genres: ['Drama', 'Fantasy'],
    has_metadata: true,
    metadata_refresh_state: 'fresh',
    theme_song_url: 'https://www.youtube.com/watch?v=uXZd_W5B7N0',
    extras: [],
    audio_tracks: [],
    subtitle_tracks: [],
    playback_target: {
      item_id: 203,
      start_ms: 74_000,
      label: 'Resume S01E01',
      display_title: 'Mock Episode',
      season_number: 1,
      episode_number: 1,
      resume: true,
    },
    restart_playback_target: {
      item_id: 203,
      start_ms: 0,
      label: 'Start season',
      display_title: 'Mock Episode',
      season_number: 1,
      episode_number: 1,
      resume: false,
    },
    hierarchy: [{ ...SHOW_201_SUMMARY }],
    children: [{ ...EPISODE_203_SUMMARY }],
  },
  {
    id: 203,
    library_id: 2,
    parent_id: 202,
    item_type: 'episode',
    display_title: 'Mock Episode',
    relative_path: 'Mock Show/Season 1/episode-01.mp4',
    media_kind: 'video',
    playable: true,
    child_count: 0,
    season_number: 1,
    episode_number: 1,
    duration_ms: 2_700_000,
    width: 1280,
    height: 720,
    modified_at: 1760923100,
    file_size: 810_612_736,
    container: 'mp4',
    bit_rate: 1_800_000,
    video_codec: 'h264',
    audio_codec: 'aac',
    metadata_json: JSON.stringify({ format: { format_name: 'mp4', duration: '2700.0' } }, null, 2),
    metadata_updated_at: 1760923100,
    poster_url: '/api/v1/items/203/artwork?kind=poster',
    tagline: 'Winter is coming.',
    overview: 'A major fantasy series entry used as mock TV metadata for the browser client.',
    genres: ['Drama', 'Fantasy'],
    release_year: 2011,
    linked_media_type: 'tv',
    has_metadata: true,
    metadata_refresh_state: 'fresh',
    theme_song_url: 'https://www.youtube.com/watch?v=uXZd_W5B7N0',
    extras: [],
    audio_tracks: [],
    subtitle_tracks: [],
    hierarchy: [{ ...SHOW_201_SUMMARY }, { ...SEASON_202_SUMMARY }],
    children: [],
  },
  {
    id: 103,
    library_id: 3,
    item_type: 'track',
    display_title: 'Mock Song',
    relative_path: 'mock-artist/mock-song.flac',
    media_kind: 'audio',
    playable: true,
    child_count: 0,
    duration_ms: 215_000,
    modified_at: 1760923000,
    file_size: 35_610_736,
    container: 'flac',
    bit_rate: 970_000,
    audio_codec: 'flac',
    metadata_json: JSON.stringify({ format: { format_name: 'flac', duration: '215.0' } }, null, 2),
    metadata_updated_at: 1760923000,
    genres: [],
    extras: [],
    audio_tracks: [],
    subtitle_tracks: [],
    hierarchy: [],
    children: [],
  },
  {
    id: 104,
    library_id: 3,
    item_type: 'track',
    display_title: 'Roadtrip Mix',
    relative_path: 'mock-artist/roadtrip-mix.mp3',
    media_kind: 'audio',
    playable: true,
    child_count: 0,
    duration_ms: 198_000,
    modified_at: 1760922900,
    file_size: 8_610_736,
    container: 'mp3',
    bit_rate: 320_000,
    audio_codec: 'mp3',
    metadata_json: JSON.stringify({ format: { format_name: 'mp3', duration: '198.0' } }, null, 2),
    metadata_updated_at: 1760922900,
    genres: [],
    extras: [],
    audio_tracks: [],
    subtitle_tracks: [],
    hierarchy: [],
    children: [],
  },
];

const collections: MediaCollectionSummary[] = [
  {
    id: 'tmdb:2344',
    provider_id: 'tmdb',
    external_id: '2344',
    name: 'The Matrix Collection',
    overview:
      'A cyberpunk science-fiction collection centered around Neo, Zion, and the war against the machines.',
    artwork_url: 'https://image.tmdb.org/t/p/w500/f89U3ADr1oiB1s9GkdPOEpXUk5H.jpg',
    backdrop_url: 'https://image.tmdb.org/t/p/w1280/icmmSD4vTTDKOq2vvdulafOGw93.jpg',
    theme_song_url: 'https://www.youtube.com/watch?v=SLBACEP6LsI',
    item_ids: [101],
    item_count: 1,
  },
];

const metadataProviders: MetadataProviderStatus[] = [
  {
    id: 'tmdb',
    display_name: 'TheMovieDB',
    description: 'Primary movie and television metadata provider for Koko.',
    supported_kinds: ['movies', 'shows'],
    requires_api_key: true,
    implemented: true,
    role: 'primary',
    extends_provider_ids: [],
    enabled: true,
    configured: true,
    language: 'en-US',
    attribution_text: 'Metadata and artwork provided by The Movie Database (TMDB).',
    attribution_url: 'https://www.themoviedb.org/',
    logo_light_url: undefined,
    logo_dark_url: undefined,
  },
  {
    id: 'tvdb',
    display_name: 'TheTVDB',
    description:
      'Alternative movie and television metadata provider with strong series and episode coverage.',
    supported_kinds: ['movies', 'shows'],
    requires_api_key: true,
    implemented: true,
    role: 'primary',
    extends_provider_ids: [],
    enabled: false,
    configured: false,
    language: 'en-US',
    attribution_text: 'Metadata and artwork provided by TheTVDB.',
    attribution_url: 'https://thetvdb.com/',
    logo_light_url: undefined,
    logo_dark_url: undefined,
  },
  {
    id: 'musicbrainz',
    display_name: 'MusicBrainz',
    description: 'Planned music metadata provider for albums, artists, and tracks.',
    supported_kinds: ['music'],
    requires_api_key: false,
    implemented: false,
    role: 'primary',
    extends_provider_ids: [],
    enabled: false,
    configured: true,
    language: 'en-US',
    attribution_text: 'MusicBrainz metadata is provided by MusicBrainz.',
    attribution_url: 'https://musicbrainz.org/',
    logo_light_url: undefined,
    logo_dark_url: undefined,
  },
  {
    id: 'themerr',
    display_name: 'ThemerrDB',
    description: 'Secondary theme-song provider for linked movie and show metadata.',
    supported_kinds: ['movies', 'shows'],
    requires_api_key: false,
    implemented: true,
    role: 'secondary',
    extends_provider_ids: ['tmdb', 'tvdb'],
    enabled: true,
    configured: true,
    language: 'en-US',
    attribution_text: 'Theme metadata provided by ThemerrDB.',
    attribution_url: 'https://app.lizardbyte.dev/ThemerrDB',
    logo_light_url: undefined,
    logo_dark_url: undefined,
  },
];

const users: MockUserRecord[] = [
  {
    id: 1,
    username: 'admin',
    password: 'adminpass',
    admin: true,
    birthday: '1990-01-01',
    profile_image_url: undefined,
    preferred_metadata_languages: ['en-US'],
  },
];

// Playback progress keyed by '<userId>:<itemId>'. Seeds make continue-watching
// non-empty for the seeded admin (user 1).
const playbackProgress = new Map<string, MockPlaybackProgress>([
  ['1:101', { position_ms: 1_260_000, duration_ms: 5_400_000, completed: false, watch_count: 0 }],
  ['1:103', { position_ms: 74_000, duration_ms: 215_000, completed: false, watch_count: 0 }],
]);

// Item metadata. 101 is literal; 201 is built via metadataMatchesWithSecondaries
// at module init (and on demand). Other items get empty matches.
const itemMetadata: Record<number, ItemMetadataResponse> = {};

const metadataSearchResults: Record<number, MetadataSearchResult[]> = {
  101: [
    {
      provider_id: 'tmdb',
      external_id: '603',
      media_type: 'movie',
      title: 'The Matrix',
      overview:
        'A computer hacker learns the true nature of reality and his role in the war against its controllers.',
      artwork_url: 'https://image.tmdb.org/t/p/w500/f89U3ADr1oiB1s9GkdPOEpXUk5H.jpg',
      backdrop_url: 'https://image.tmdb.org/t/p/w1280/icmmSD4vTTDKOq2vvdulafOGw93.jpg',
      release_year: 1999,
    },
  ],
  203: [
    {
      provider_id: 'tmdb',
      external_id: '1399',
      media_type: 'tv',
      title: 'Game of Thrones',
      overview:
        'Nine noble families wage war against each other in order to gain control over the mythical land of Westeros.',
      artwork_url: 'https://image.tmdb.org/t/p/w500/u3bZgnGQ9T01sWNhyveQz0wH0Hl.jpg',
      backdrop_url: 'https://image.tmdb.org/t/p/w1280/suopoADq0k8YZr4dQXcU6pToj6s.jpg',
      release_year: 2011,
    },
  ],
};

let settings: SettingsSnapshot = {
  general: { data_dir: 'C:/Users/Mock/AppData/Local/Koko/data' },
  media: {
    missing_item_auto_delete_days: null,
    libraries: [
      {
        name: 'Movies',
        path: 'C:/Media/Movies',
        paths: ['C:/Media/Movies', 'D:/Overflow/Movies'],
        recursive: true,
        kind: 'movies',
        scanner: 'auto',
        metadata_providers: ['tmdb'],
        metadata_language_mode: 'auto',
        metadata_languages: ['en-US'],
        allowed_user_ids: [],
      },
      {
        name: 'Shows',
        path: 'C:/Media/Shows',
        paths: ['C:/Media/Shows'],
        recursive: true,
        kind: 'shows',
        scanner: 'auto',
        metadata_providers: ['tmdb'],
        metadata_language_mode: 'auto',
        metadata_languages: ['en-US', 'ja-JP'],
        allowed_user_ids: [],
      },
      {
        name: 'Music',
        path: 'C:/Media/Music',
        paths: ['C:/Media/Music'],
        recursive: true,
        kind: 'music',
        scanner: 'auto',
        metadata_providers: [],
        metadata_language_mode: 'auto',
        metadata_languages: ['en-US'],
        allowed_user_ids: [],
      },
    ],
  },
  metadata: {
    providers: [
      {
        id: 'tmdb',
        enabled: true,
        api_key: null,
        api_key_configured: true,
        language: 'en-US',
        rate_limit_per_second: 4,
        retry_attempts: 3,
        retry_backoff_ms: 1000,
      },
      {
        id: 'tvdb',
        enabled: false,
        api_key: null,
        api_key_configured: false,
        language: 'en-US',
        rate_limit_per_second: 4,
        retry_attempts: 3,
        retry_backoff_ms: 1000,
      },
      {
        id: 'themerr',
        enabled: true,
        api_key: null,
        api_key_configured: false,
        language: 'en-US',
        rate_limit_per_second: 4,
        retry_attempts: 3,
        retry_backoff_ms: 1000,
      },
    ],
    refresh_interval_days: 30,
  },
  scheduled_tasks: {
    enabled: true,
    window: {
      start_time: '02:00',
      stop_time: '06:00',
      weekdays: [
        'monday',
        'tuesday',
        'wednesday',
        'thursday',
        'friday',
        'saturday',
        'sunday',
      ],
    },
    metadata_refresh: { enabled: true },
    trash_cleanup: { enabled: false, missing_item_auto_delete_days: null, interval_days: 1 },
    database_maintenance: { enabled: true, interval_days: 7 },
  },
  server: {
    use_https: false,
    address: '127.0.0.1',
    port: 9191,
    cert_path: 'cert.pem',
    key_path: 'key.pem',
    use_custom_certs: false,
  },
  ffmpeg: { ffmpeg_path: 'ffmpeg', ffprobe_path: 'ffprobe' },
};

// Counters for created libraries / users.
let nextLibraryId = 4;
let nextUserId = 2;

// ---------------------------------------------------------------------------
// Stateful helpers (the 4 behavioral quirks)
// ---------------------------------------------------------------------------

const AUTH_TOKEN_STORAGE_KEY = 'koko-client-web-auth-token';

function activeMockUserId(): number | undefined {
  const token = globalThis.localStorage?.getItem(AUTH_TOKEN_STORAGE_KEY)?.trim();
  if (!token || !token.startsWith('mock-token-')) {
    return undefined;
  }
  const id = Number(token.slice('mock-token-'.length));
  return Number.isFinite(id) ? id : undefined;
}

/** Quirk #1: overlay playback progress onto an item only for the active user. */
function applyMockPlaybackProgress<T extends MediaItemSummary>(item: T): T {
  const userId = activeMockUserId();
  if (userId === undefined) {
    return item;
  }
  const progress = playbackProgress.get(`${userId}:${item.id}`);
  if (!progress) {
    return item;
  }
  return {
    ...item,
    playback_position_ms: progress.position_ms,
    playback_duration_ms: progress.duration_ms,
    playback_completed: progress.completed,
    watch_count: progress.watch_count,
    last_watched_at: progress.last_watched_at,
  };
}

/** Quirk #2: recompute library metadata-refresh counters from item state. */
function syncAllMockLibraryRefreshProgress(): void {
  for (const library of libraries) {
    const libraryItems = items.filter(
      (item) => item.library_id === library.id && item.has_metadata,
    );
    const total = libraryItems.length;
    const pending = libraryItems.filter((item) => item.metadata_refresh_state === 'pending').length;
    const failed = libraryItems.filter((item) => item.metadata_refresh_state === 'error').length;
    library.metadata_refresh_total = total;
    library.metadata_refresh_pending = pending;
    library.metadata_refresh_completed = total - pending - failed;
    library.metadata_refresh_failed = failed;
  }
}

// ---------------------------------------------------------------------------
// Metadata match builders (verbatim semantics from vanilla mockApi.ts)
// ---------------------------------------------------------------------------

function themerrSecondaryMatch(
  item: MediaItemDetail,
  primaryMatch: ItemMetadataMatch,
  secondaryId: number,
): ItemMetadataMatch {
  const isShow = item.item_type === 'show' || primaryMatch.media_type === 'tv';
  const externalId = isShow
    ? `show:tmdb:${primaryMatch.external_id}`
    : `movie:tmdb:${primaryMatch.external_id}`;
  const themeSongUrl = isShow
    ? 'https://www.youtube.com/watch?v=uXZd_W5B7N0'
    : 'https://www.youtube.com/watch?v=SLBACEP6LsI';
  return {
    id: secondaryId,
    provider_id: 'themerr',
    external_id: externalId,
    media_type: isShow ? 'show' : 'movie',
    relation_kind: 'secondary',
    match_state: 'linked',
    theme_song_url: themeSongUrl,
    genres: [],
    people: [],
    locale_key: 'en-US',
    refresh_state: 'fresh',
    last_refreshed_at: 1760923200,
    updated_at: 1760923200,
  };
}

function metadataMatchesWithSecondaries(
  item: MediaItemDetail,
  primaryMatch: ItemMetadataMatch,
): ItemMetadataMatch[] {
  if (item.item_type === 'movie' || item.item_type === 'show') {
    return [primaryMatch, themerrSecondaryMatch(item, primaryMatch, primaryMatch.id + 1)];
  }
  return [primaryMatch];
}

function buildItemMetadata(): void {
  // Item 101 — literal rich match (Matrix, full cast).
  itemMetadata[101] = {
    item_id: 101,
    providers: metadataProviders.map((provider) => ({ ...provider })),
    matches: [
      {
        id: 1,
        provider_id: 'tmdb',
        external_id: '603',
        title: 'The Matrix',
        overview:
          'A computer hacker learns the true nature of reality and his role in the war against its controllers.',
        artwork_url: 'https://image.tmdb.org/t/p/w500/f89U3ADr1oiB1s9GkdPOEpXUk5H.jpg',
        backdrop_url: 'https://image.tmdb.org/t/p/w1280/icmmSD4vTTDKOq2vvdulafOGw93.jpg',
        release_year: 1999,
        media_type: 'movie',
        relation_kind: 'primary',
        match_state: 'linked',
        trailer_title: 'Official Trailer',
        trailer_url: 'https://www.youtube.com/watch?v=vKQi3bBA1y8',
        genres: ['Action', 'Science Fiction'],
        people: [
          {
            id: 1,
            person_id: 1,
            external_id: '6384',
            name: 'Keanu Reeves',
            role: 'Actor',
            department: 'Cast',
            character_name: 'Neo',
            image_url: 'https://image.tmdb.org/t/p/w185/4D0PpNI0kmP58hgrwGC3wCjxhnm.jpg',
            profile_url: 'https://www.themoviedb.org/person/6384',
            sort_order: 0,
          },
          {
            id: 2,
            person_id: 2,
            external_id: '2975',
            name: 'Laurence Fishburne',
            role: 'Actor',
            department: 'Cast',
            character_name: 'Morpheus',
            image_url: 'https://image.tmdb.org/t/p/w185/8suOhUmPbfKqDQ17jQ1Gy0mI3P4.jpg',
            profile_url: 'https://www.themoviedb.org/person/2975',
            sort_order: 1,
          },
          {
            id: 3,
            person_id: 3,
            external_id: '9340',
            name: 'Carrie-Anne Moss',
            role: 'Actor',
            department: 'Cast',
            character_name: 'Trinity',
            image_url: 'https://image.tmdb.org/t/p/w185/xD4jTA3KmVp5Rq3aHcymL9DUGjD.jpg',
            profile_url: 'https://www.themoviedb.org/person/9340',
            sort_order: 2,
          },
          {
            id: 4,
            person_id: 4,
            external_id: '9339',
            name: 'Lana Wachowski',
            role: 'Director',
            department: 'Directing',
            profile_url: 'https://www.themoviedb.org/person/9339',
            sort_order: 10000,
          },
          {
            id: 5,
            person_id: 5,
            external_id: '9341',
            name: 'Lilly Wachowski',
            role: 'Director',
            department: 'Directing',
            profile_url: 'https://www.themoviedb.org/person/9341',
            sort_order: 10001,
          },
        ],
        locale_key: 'en-US',
        refresh_state: 'fresh',
        last_refreshed_at: 1760923200,
        updated_at: 1760923200,
      },
      {
        id: 2,
        provider_id: 'themerr',
        external_id: 'movie:tmdb:603',
        media_type: 'movie',
        relation_kind: 'secondary',
        match_state: 'linked',
        theme_song_url: 'https://www.youtube.com/watch?v=SLBACEP6LsI',
        genres: [],
        people: [],
        locale_key: 'en-US',
        refresh_state: 'fresh',
        last_refreshed_at: 1760923200,
        updated_at: 1760923200,
      },
    ],
  };

  // Item 201 — built dynamically with secondaries (Game of Thrones).
  const show201 = items.find((item) => item.id === 201)!;
  const primary201: ItemMetadataMatch = {
    id: 3,
    provider_id: 'tmdb',
    external_id: '1399',
    title: 'Game of Thrones',
    overview:
      'Nine noble families wage war against each other in order to gain control over the mythical land of Westeros.',
    artwork_url: 'https://image.tmdb.org/t/p/w500/u3bZgnGQ9T01sWNhyveQz0wH0Hl.jpg',
    backdrop_url: 'https://image.tmdb.org/t/p/w1280/suopoADq0k8YZr4dQXcU6pToj6s.jpg',
    release_year: 2011,
    media_type: 'tv',
    relation_kind: 'primary',
    match_state: 'linked',
    genres: ['Drama', 'Fantasy'],
    people: [],
    locale_key: 'en-US',
    refresh_state: 'fresh',
    last_refreshed_at: 1760923200,
    updated_at: 1760923200,
  };
  itemMetadata[201] = {
    item_id: 201,
    providers: metadataProviders.map((provider) => ({ ...provider })),
    matches: metadataMatchesWithSecondaries(show201, primary201),
  };

  // Items with empty matches.
  for (const itemId of [202, 203, 103, 104]) {
    itemMetadata[itemId] = {
      item_id: itemId,
      providers: metadataProviders.map((provider) => ({ ...provider })),
      matches: [],
    };
  }
}

buildItemMetadata();

// ---------------------------------------------------------------------------
// Public mock getters (mirror vanilla getMock* signatures)
// ---------------------------------------------------------------------------

function toUserSummary(user: MockUserRecord): BootstrapUser {
  return {
    id: user.id,
    username: user.username,
    admin: user.admin,
    birthday: user.birthday,
    profile_image_url: user.profile_image_url,
    preferred_metadata_languages: user.preferred_metadata_languages,
  };
}

function cloneWithoutSecrets(user: MockUserRecord): BootstrapUser {
  return toUserSummary(user);
}

function mockProfileImageUrl(upload: { mime_type: string; data_base64: string }): string {
  return `data:${upload.mime_type};base64,${upload.data_base64}`;
}

export function getMockCapabilities(): ServerCapabilities {
  return {
    app_name: 'Koko',
    version: '0.0.0-dev',
    server_url: 'http://127.0.0.1:9191',
    https_enabled: false,
    libraries_configured: libraries.length,
    api_versions: ['v1'],
    transcoding: {
      ffmpeg: { available: true, version: 'ffmpeg mock build' },
      ffprobe: { available: true, version: 'ffprobe mock build' },
    },
  };
}

export function getMockBootstrap(): AppBootstrapResponse {
  const userId = activeMockUserId();
  const current = userId !== undefined ? users.find((user) => user.id === userId) : undefined;
  return {
    has_users: users.length > 0,
    current_user: current ? cloneWithoutSecrets(current) : undefined,
  };
}

export function getMockUsers(): BootstrapUser[] {
  return users.map(cloneWithoutSecrets);
}

export function loginMockUser(request: { username: string; password: string }): TokenResponse {
  const user = users.find(
    (entry) => entry.username.toLowerCase() === request.username.toLowerCase(),
  );
  if (!user || user.password !== request.password) {
    throw new Error('401 Unauthorized');
  }
  return { token: `mock-token-${user.id}` };
}

export function createMockUser(request: CreateUserRequest): string {
  // First user is always admin; subsequent require an active admin.
  const isFirst = users.length === 0;
  if (!isFirst) {
    const userId = activeMockUserId();
    if (userId === undefined) {
      throw new Error('401 Unauthorized');
    }
    const actor = users.find((user) => user.id === userId);
    if (!actor?.admin) {
      throw new Error('403 Forbidden');
    }
  }
  if (
    users.some((user) => user.username.toLowerCase() === request.username.trim().toLowerCase())
  ) {
    throw new Error('409 Conflict');
  }
  const id = nextUserId++;
  users.push({
    id,
    username: request.username.trim(),
    password: request.password,
    pin: request.pin,
    admin: isFirst ? true : request.admin,
    birthday: request.birthday,
    profile_image_url: request.profile_image_upload
      ? mockProfileImageUrl(request.profile_image_upload)
      : undefined,
    preferred_metadata_languages: request.preferred_metadata_languages ?? ['en-US'],
  });
  return 'User created';
}

export function updateMockUser(userId: number, request: UpdateUserRequest): BootstrapUser {
  const actorId = activeMockUserId();
  if (actorId === undefined) {
    throw new Error('401 Unauthorized');
  }
  const actor = users.find((user) => user.id === actorId);
  if (!actor?.admin) {
    throw new Error('403 Forbidden');
  }
  const target = users.find((user) => user.id === userId);
  if (!target) {
    throw new Error('404 Not Found');
  }
  if (!request.username.trim()) {
    throw new Error('400 Bad Request');
  }
  if (
    users.some(
      (user) =>
        user.id !== userId && user.username.toLowerCase() === request.username.trim().toLowerCase(),
    )
  ) {
    throw new Error('409 Conflict');
  }
  // Refuse to demote the last admin.
  const admins = users.filter((user) => user.admin);
  if (target.admin && !request.admin && admins.length === 1) {
    throw new Error('400 Bad Request');
  }
  target.username = request.username.trim();
  target.admin = request.admin;
  target.birthday = request.birthday;
  if (request.preferred_metadata_languages) {
    target.preferred_metadata_languages = request.preferred_metadata_languages;
  }
  if (request.remove_profile_image) {
    target.profile_image_url = undefined;
  } else if (request.profile_image_upload) {
    target.profile_image_url = mockProfileImageUrl(request.profile_image_upload);
  }
  return cloneWithoutSecrets(target);
}

export function getMockLibraries(): MediaLibrary[] {
  syncAllMockLibraryRefreshProgress();
  return libraries.map((library) => ({ ...library }));
}

export function getMockHome(libraryId?: number): MediaHome {
  const filtered = getMockItems(libraryId);
  const continueWatching = filtered.filter(
    (item) =>
      item.playback_position_ms &&
      item.playback_position_ms > 0 &&
      !item.playback_completed,
  );
  const recentlyAdded = [...filtered].toSorted((a, b) => (b.modified_at ?? 0) - (a.modified_at ?? 0));
  const continueIds = new Set(continueWatching.map((item) => item.id));
  const recommended = filtered.filter((item) => !continueIds.has(item.id));
  return {
    library_id: libraryId,
    shelves: [
      { id: 'continue_watching', title: 'Continue watching', items: continueWatching },
      { id: 'recently_added', title: 'Recently added', items: recentlyAdded },
      { id: 'recommended', title: 'Recommended', items: recommended },
    ],
    collections: collections.filter((collection) =>
      collection.item_ids.some((id) => filtered.some((item) => item.id === id)),
    ),
  };
}

export function getMockItems(libraryId?: number): MediaItemSummary[] {
  return items
    .filter((item) => (libraryId === undefined ? true : item.library_id === libraryId))
    .map(summarizeItem)
    .map(applyMockPlaybackProgress);
}

export function getMockItem(itemId: number): MediaItemDetail {
  const item = items.find((entry) => entry.id === itemId);
  if (!item) {
    throw new Error('404 Not Found');
  }
  return applyMockPlaybackProgress(structuredClone(item));
}

export function getMockItemMetadata(itemId: number): ItemMetadataResponse {
  const entry = itemMetadata[itemId];
  if (!entry) {
    throw new Error('404 Not Found');
  }
  return structuredClone(entry);
}

export function getMockPerson(personId: number): MetadataPersonResponse {
  const credits: MetadataPersonItemCredit[] = [];
  let personCredit: {
    person_id: number;
    external_id?: string;
    name: string;
    role?: string;
    department?: string;
    character_name?: string;
    profile_url?: string;
    image_url?: string;
  } | null = null;

  for (const response of Object.values(itemMetadata)) {
    for (const match of response.matches) {
      for (const person of match.people) {
        if (person.person_id === personId) {
          personCredit = person;
          const item = items.find((entry) => entry.id === response.item_id);
          if (item) {
            credits.push({
              credit: {
                id: person.id,
                metadata_link_id: match.id,
                media_item_id: response.item_id,
                role: person.role,
                department: person.department,
                character_name: person.character_name,
                sort_order: person.sort_order,
              },
              item: summarizeItem(item),
              hierarchy: [],
            });
          }
        }
      }
    }
  }

  if (!personCredit) {
    throw new Error('404 Not Found');
  }

  const person = {
    id: personId,
    provider_id: 'tmdb',
    external_id: personCredit.external_id,
    locale_key: 'en-US',
    name: personCredit.name,
    known_for: ['The Matrix'],
    biography:
      personCredit.name === 'Keanu Reeves'
        ? 'Canadian actor known for action films, science fiction, and understated dramatic work.'
        : undefined,
    gender: personCredit.name === 'Carrie-Anne Moss' ? 'Female' : 'Male',
    birthday: personCredit.name === 'Keanu Reeves' ? '1964-09-02' : undefined,
    birth_place: personCredit.name === 'Keanu Reeves' ? 'Beirut, Lebanon' : undefined,
    profile_url: personCredit.profile_url,
    image_url: personCredit.image_url,
  };

  return { person, credits };
}

export function getMockSettings(): SettingsResponse {
  return {
    settings: structuredClone(settings),
    settings_path: 'C:/Users/Mock/AppData/Local/Koko/settings.yml',
  };
}

export function updateMockSettings(next: SettingsSnapshot): SettingsResponse {
  settings = structuredClone(next);
  return getMockSettings();
}

export function getMockMetadataProviders(): MetadataProviderStatus[] {
  return metadataProviders.map((provider) => ({ ...provider }));
}

export function getMockSystemActivities(): SystemActivitiesResponse {
  const now = Math.floor(Date.now() / 1000);
  const activities = [];
  for (const library of libraries) {
    const pendingItems = items.filter(
      (item) =>
        item.library_id === library.id &&
        item.has_metadata &&
        item.metadata_refresh_state === 'pending',
    );
    if (pendingItems.length > 0) {
      activities.push({
        id: `mock-activity-library-${library.id}`,
        category: 'metadata_refresh',
        scope: 'library',
        source: 'mock_refresh',
        state: 'running',
        label: `Refresh metadata for ${library.name}`,
        provider_id: 'tmdb',
        library_id: library.id,
        item_ids: pendingItems.map((item) => item.id),
        total_items: pendingItems.length,
        completed_items: 0,
        failed_items: 0,
        queued_at: now,
        started_at: now,
        updated_at: now,
      });
    }
  }
  return { generated_at: now, activities };
}

export function getMockPlayback(itemId: number): PlaybackDecision {
  const item = items.find((entry) => entry.id === itemId);
  return {
    item_id: itemId,
    can_direct_play: item?.playable ?? false,
    transcode_required: false,
    reason: item?.playable ? 'Ready to play.' : 'Item is not directly playable.',
    stream_url: item?.playable ? `/api/v1/items/${itemId}/stream` : undefined,
    mime_type: item?.container === 'mp4' ? 'video/mp4' : undefined,
    video_transcode_required: false,
    audio_transcode_required: false,
  };
}

export function createMockPlaybackSession(
  request: CreateSessionRequest,
): PlaybackSession {
  return {
    session_id: `mock-session-${request.item_id}-${Date.now()}`,
    item_id: request.item_id,
    client_profile: request.client_profile,
    decision: getMockPlayback(request.item_id),
    created_at: Math.floor(Date.now() / 1000),
  };
}

export function updateMockPlaybackProgress(
  itemId: number,
  payload: PlaybackProgressRequest,
): void {
  const userId = activeMockUserId();
  if (userId === undefined) {
    return;
  }
  const key = `${userId}:${itemId}`;
  const existing = playbackProgress.get(key);
  const completedTransition = payload.completed && !existing?.completed;
  playbackProgress.set(key, {
    ...payload,
    watch_count: completedTransition ? (existing?.watch_count ?? 0) + 1 : existing?.watch_count ?? 0,
    last_watched_at: completedTransition ? Math.floor(Date.now() / 1000) : existing?.last_watched_at,
  });
}

export function searchMockItems(query: string): MediaSearchResult[] {
  const normalized = query.trim().toLowerCase();
  if (!normalized) {
    return [];
  }
  const results: MediaSearchResult[] = [];
  for (const item of getMockItems()) {
    if (item.display_title.toLowerCase().includes(normalized)) {
      results.push({ result_type: 'item', item });
    }
  }
  for (const collection of collections) {
    if (collection.name.toLowerCase().includes(normalized)) {
      results.push({ result_type: 'collection', collection });
    }
  }
  return results;
}

export function searchMockItemMetadata(
  itemId: number,
  options?: string | MetadataSearchOptions,
): MetadataSearchResult[] {
  const opts: MetadataSearchOptions = typeof options === 'string' ? { query: options } : options ?? {};
  const candidates = metadataSearchResults[itemId] ?? [];
  if (!opts.query) {
    return candidates;
  }
  const normalized = opts.query.toLowerCase();
  return candidates.filter(
    (candidate) =>
      candidate.title.toLowerCase().includes(normalized) ||
      (candidate.overview?.toLowerCase().includes(normalized) ?? false),
  );
}

export function linkMockItemMetadata(
  itemId: number,
  request: LinkMetadataRequest,
): ItemMetadataMatch {
  const candidates = metadataSearchResults[itemId] ?? [];
  const candidate = candidates.find(
    (entry) =>
      entry.provider_id === request.provider_id && entry.external_id === request.external_id,
  );
  if (!candidate) {
    throw new Error('404 Not Found');
  }
  const existing = itemMetadata[itemId];
  const newMatchId = (existing?.matches.reduce((max, match) => Math.max(max, match.id), 0) ?? 0) + 1;
  const item = items.find((entry) => entry.id === itemId);
  const primaryMatch: ItemMetadataMatch = {
    id: newMatchId,
    provider_id: candidate.provider_id,
    external_id: candidate.external_id,
    title: candidate.title,
    overview: candidate.overview,
    artwork_url: candidate.artwork_url,
    backdrop_url: candidate.backdrop_url,
    release_year: candidate.release_year,
    media_type: candidate.media_type,
    relation_kind: 'primary',
    match_state: 'linked',
    genres: [],
    people: [],
    locale_key: 'en-US',
    refresh_state: 'fresh',
    last_refreshed_at: Math.floor(Date.now() / 1000),
    updated_at: Math.floor(Date.now() / 1000),
  };
  const matches = item
    ? metadataMatchesWithSecondaries(item, primaryMatch)
    : [primaryMatch];
  if (item) {
    item.display_title = candidate.title;
    item.overview = candidate.overview;
    item.release_year = candidate.release_year;
    item.linked_media_type = candidate.media_type;
  }
  itemMetadata[itemId] = {
    item_id: itemId,
    providers: metadataProviders.map((provider) => ({ ...provider })),
    matches,
  };
  return structuredClone(primaryMatch);
}

export function refreshMockItemMetadata(itemId: number): ItemMetadataMatch {
  const response = itemMetadata[itemId];
  const match =
    response.matches.find((entry) => entry.relation_kind === 'primary') ?? response.matches[0];
  if (match) {
    match.refresh_state = 'pending';
    const item = items.find((entry) => entry.id === itemId);
    if (item) {
      item.metadata_refresh_state = 'pending';
    }
    syncAllMockLibraryRefreshProgress();
    // Quirk #4: async flip back to fresh after 900ms.
    setTimeout(() => {
      const candidate = metadataSearchResults[itemId]?.[0];
      if (candidate && item) {
        match.title = candidate.title;
        match.overview = candidate.overview;
        match.release_year = candidate.release_year;
        match.refresh_state = 'fresh';
        item.display_title = candidate.title;
        item.overview = candidate.overview;
        item.release_year = candidate.release_year;
        item.linked_media_type = candidate.media_type;
        item.metadata_refresh_state = 'fresh';
        item.artwork_updated_at = Math.floor(Date.now() / 1000);
        match.updated_at = item.artwork_updated_at;
        match.last_refreshed_at = item.artwork_updated_at;
        syncAllMockLibraryRefreshProgress();
      }
    }, 900);
  }
  return structuredClone(match);
}

export function refreshMockLibraryMetadata(libraryId: number): MediaLibrary {
  const refreshable = items.filter(
    (item) => item.library_id === libraryId && item.has_metadata,
  );
  for (const item of refreshable) {
    item.metadata_refresh_state = 'pending';
  }
  syncAllMockLibraryRefreshProgress();
  // Quirk #4: async flip back to fresh after 1200ms.
  setTimeout(() => {
    for (const item of refreshable) {
      item.metadata_refresh_state = 'fresh';
      item.artwork_updated_at = Math.floor(Date.now() / 1000);
    }
    syncAllMockLibraryRefreshProgress();
  }, 1200);
  const library = libraries.find((entry) => entry.id === libraryId);
  return library ? { ...library } : { ...libraries[0] };
}

export function addMockLibrary(library: MediaLibrarySettings): SettingsResponse {
  const normalized: MediaLibrarySettings = {
    name: library.name.trim(),
    path: library.path.trim(),
    paths: library.paths.length ? library.paths : [library.path.trim()],
    recursive: library.recursive,
    kind: library.kind,
    scanner: library.scanner || 'auto',
    metadata_providers: library.metadata_providers,
    metadata_language_mode: library.metadata_language_mode,
    metadata_languages: library.metadata_languages.length ? library.metadata_languages : ['en-US'],
    allowed_user_ids: library.allowed_user_ids ?? [],
  };
  settings.media.libraries.push(normalized);
  const now = Math.floor(Date.now() / 1000);
  libraries.push({
    id: nextLibraryId++,
    name: normalized.name,
    path: normalized.path,
    paths: normalized.paths,
    recursive: normalized.recursive,
    kind: normalized.kind,
    scanner: normalized.kind,
    metadata_providers: normalized.metadata_providers,
    metadata_language_mode: normalized.metadata_language_mode,
    metadata_languages: normalized.metadata_languages,
    status: 'available',
    scan_revision: 1,
    last_scanned_at: now,
    total_files: 0,
    video_files: 0,
    audio_files: 0,
    image_files: 0,
    book_files: 0,
    other_files: 0,
    metadata_refresh_total: 0,
    metadata_refresh_pending: 0,
    metadata_refresh_completed: 0,
    metadata_refresh_failed: 0,
    missing_files: 0,
    missing_items: 0,
  });
  syncAllMockLibraryRefreshProgress();
  return getMockSettings();
}

export function removeMockLibrary(libraryIndex: number): SettingsResponse {
  const settingsLibrary = settings.media.libraries[libraryIndex];
  settings.media.libraries.splice(libraryIndex, 1);
  if (settingsLibrary) {
    const runtimeIndex = libraries.findIndex(
      (library) =>
        library.name === settingsLibrary.name && library.path === settingsLibrary.path,
    );
    if (runtimeIndex >= 0) {
      libraries.splice(runtimeIndex, 1);
    }
  }
  return getMockSettings();
}

export function deleteMockMissingItems(libraryId: number): MissingItemsCleanupResponse {
  let deletedFiles = 0;
  let deletedItems = 0;
  for (let i = items.length - 1; i >= 0; i--) {
    if (items[i].library_id === libraryId && items[i].missing_since) {
      deletedFiles += 1;
      deletedItems += 1;
      items.splice(i, 1);
    }
  }
  let removedCollectionItems = 0;
  for (const collection of collections) {
    const before = collection.item_ids.length;
    collection.item_ids = collection.item_ids.filter((id) =>
      items.some((item) => item.id === id),
    );
    removedCollectionItems += before - collection.item_ids.length;
    collection.item_count = collection.item_ids.length;
  }
  const library = libraries.find((entry) => entry.id === libraryId);
  if (library) {
    library.missing_files = 0;
    library.missing_items = 0;
  }
  return {
    library_id: libraryId,
    deleted_files: deletedFiles,
    deleted_items: deletedItems,
    removed_collection_items: removedCollectionItems,
    library: library ? { ...library } : { ...libraries[0] },
  };
}

export function clearMockMetadataCache(): MetadataCacheClearResponse {
  return { removed_files: 0 };
}

export function runMockScheduledTask(taskId: ScheduledTaskId): ScheduledTaskRunResponse {
  return {
    task_id: taskId,
    started: true,
    message: `${taskId.replace(/_/g, ' ')} started`,
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
  const entries = [
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

// ---------------------------------------------------------------------------
// Dispatch — route method + path to the right mock getter
// ---------------------------------------------------------------------------

class MockError extends Error {
  status: number;
  constructor(status: number, message: string) {
    super(message);
    this.status = status;
  }
}

function parsePath(path: string): URL {
  return new URL(path, 'http://koko.local');
}

function getMockGetResponse(path: string): unknown {
  const url = parsePath(path);
  const pathname = url.pathname;
  switch (pathname) {
    case '/api/v1/system/capabilities':
      return getMockCapabilities();
    case '/api/v1/bootstrap':
      return getMockBootstrap();
    case '/api/v1/users':
      return getMockUsers();
    case '/api/v1/libraries':
      return getMockLibraries();
    case '/api/v1/metadata/providers':
      return getMockMetadataProviders();
    case '/api/v1/system/activities':
      return getMockSystemActivities();
    case '/api/v1/settings':
      return getMockSettings();
    case '/api/v1/settings/logs': {
      const params = url.searchParams;
      return getMockLogs(
        params.get('level') ?? undefined,
        params.get('module') ?? undefined,
        params.get('search') ?? undefined,
        params.get('since') ?? undefined,
        params.get('until') ?? undefined,
        params.get('limit') ? Number(params.get('limit')) : undefined,
      );
    }
    case '/api/v1/home':
      return getMockHome(
        paramsNumber(url.searchParams, 'library_id'),
      );
    case '/api/v1/items':
      return getMockItems(paramsNumber(url.searchParams, 'library_id'));
    case '/api/v1/search':
      return searchMockItems(url.searchParams.get('query') ?? '');
  }

  let match: RegExpMatchArray | null;
  if ((match = pathname.match(/^\/api\/v1\/items\/(\d+)\/metadata\/search$/))) {
    return searchMockItemMetadata(
      Number(match[1]),
      url.searchParams.get('query') ?? undefined,
    );
  }
  if ((match = pathname.match(/^\/api\/v1\/items\/(\d+)\/metadata$/))) {
    return getMockItemMetadata(Number(match[1]));
  }
  if ((match = pathname.match(/^\/api\/v1\/items\/(\d+)\/playback$/))) {
    return getMockPlayback(Number(match[1]));
  }
  if ((match = pathname.match(/^\/api\/v1\/people\/(\d+)$/))) {
    return getMockPerson(Number(match[1]));
  }
  if ((match = pathname.match(/^\/api\/v1\/items\/(\d+)$/))) {
    return getMockItem(Number(match[1]));
  }
  if ((match = pathname.match(/^\/api\/v1\/sessions\/([^/]+)\/stream$/))) {
    throw new MockError(501, 'Not Implemented (mock streaming not fully supported)');
  }
  throw new MockError(404, `No mock response is defined for GET ${pathname}`);
}

function getMockPostResponse(path: string, body: unknown): unknown {
  const url = parsePath(path);
  const pathname = url.pathname;
  switch (pathname) {
    case '/login':
      return loginMockUser(body as { username: string; password: string });
    case '/create_user':
      return createMockUser(body as CreateUserRequest);
    case '/api/v1/settings/libraries':
      return addMockLibrary((body as { library: MediaLibrarySettings }).library);
    case '/api/v1/settings/metadata-cache/clear':
      return clearMockMetadataCache();
    case '/api/v1/sessions':
      return createMockPlaybackSession(body as CreateSessionRequest);
  }

  let match: RegExpMatchArray | null;
  if ((match = pathname.match(/^\/api\/v1\/scheduled-tasks\/([^/]+)\/run$/))) {
    return runMockScheduledTask(match[1] as ScheduledTaskId);
  }
  if ((match = pathname.match(/^\/api\/v1\/items\/(\d+)\/progress$/))) {
    updateMockPlaybackProgress(Number(match[1]), body as PlaybackProgressRequest);
    return undefined;
  }
  if ((match = pathname.match(/^\/api\/v1\/items\/(\d+)\/metadata\/link$/))) {
    return linkMockItemMetadata(Number(match[1]), body as LinkMetadataRequest);
  }
  if ((match = pathname.match(/^\/api\/v1\/items\/(\d+)\/metadata\/refresh$/))) {
    return refreshMockItemMetadata(Number(match[1]));
  }
  if ((match = pathname.match(/^\/api\/v1\/libraries\/(\d+)\/metadata\/refresh$/))) {
    return refreshMockLibraryMetadata(Number(match[1]));
  }
  if ((match = pathname.match(/^\/api\/v1\/libraries\/(\d+)\/scan$/))) {
    // Mock treats scan like a metadata refresh.
    return refreshMockLibraryMetadata(Number(match[1]));
  }
  throw new MockError(404, `No mock response is defined for POST ${pathname}`);
}

function getMockPutResponse(path: string, body: unknown): unknown {
  const url = parsePath(path);
  const pathname = url.pathname;
  if (pathname === '/api/v1/settings') {
    return updateMockSettings(body as SettingsSnapshot);
  }
  const match = pathname.match(/^\/api\/v1\/users\/(\d+)$/);
  if (match) {
    return updateMockUser(Number(match[1]), body as UpdateUserRequest);
  }
  throw new MockError(404, `No mock response is defined for PUT ${pathname}`);
}

function getMockDeleteResponse(path: string): unknown {
  const url = parsePath(path);
  const pathname = url.pathname;
  let match: RegExpMatchArray | null;
  if ((match = pathname.match(/^\/api\/v1\/settings\/libraries\/(\d+)$/))) {
    return removeMockLibrary(Number(match[1]));
  }
  if ((match = pathname.match(/^\/api\/v1\/libraries\/(\d+)\/missing$/))) {
    return deleteMockMissingItems(Number(match[1]));
  }
  if ((match = pathname.match(/^\/api\/v1\/sessions\/([^/]+)$/))) {
    return undefined;
  }
  throw new MockError(404, `No mock response is defined for DELETE ${pathname}`);
}

function paramsNumber(params: URLSearchParams, key: string): number | undefined {
  const value = params.get(key);
  if (value === null) return undefined;
  const parsed = Number(value);
  return Number.isFinite(parsed) ? parsed : undefined;
}

/** Dispatch entry point used by api.ts when in mock mode. */
export async function dispatch<T>(method: string, path: string, body?: unknown): Promise<T> {
  // Defer to a microtask so the call is async like a real fetch.
  await Promise.resolve();
  let response: unknown;
  try {
    switch (method.toUpperCase()) {
      case 'GET':
        response = getMockGetResponse(path);
        break;
      case 'POST':
        response = getMockPostResponse(path, body);
        break;
      case 'PUT':
        response = getMockPutResponse(path, body);
        break;
      case 'DELETE':
        response = getMockDeleteResponse(path);
        break;
      default:
        throw new MockError(405, `Method ${method} not allowed`);
    }
  } catch (err) {
    if (err instanceof MockError) {
      throw new Error(`${err.status} ${err.message}`);
    }
    throw err;
  }
  return response as T;
}
