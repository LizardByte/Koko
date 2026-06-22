import type {
  AppBootstrapResponse,
  BootstrapUser,
  CreateUserRequest,
  ItemMetadataMatch,
  ItemMetadataPerson,
  ItemMetadataResponse,
  LoginRequest,
  LinkMetadataRequest,
  MediaCollectionSummary,
  MediaHome,
  MediaItemDetail,
  MediaItemSummary,
  MediaLibrary,
  MediaLibrarySettings,
  MediaSearchResult,
  MissingItemsCleanupResponse,
  MetadataProviderStatus,
  MetadataPersonItemCredit,
  MetadataPersonResponse,
  MetadataSearchResult,
  LogEntriesResponse,
  PlaybackDecision,
  PlaybackProgressRequest,
  ScheduledTaskId,
  ScheduledTaskRunResponse,
  ServerCapabilities,
  SettingsResponse,
  SettingsSnapshot,
  SystemActivity,
  SystemActivitiesResponse,
  TokenResponse,
  UpdateUserRequest,
} from './api';

let nextLibraryId = 4;
let nextUserId = 2;
const AUTH_TOKEN_STORAGE_KEY = 'koko-client-web-auth-token';

interface MockUserRecord extends BootstrapUser {
  password: string;
  pin?: string;
}

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
    metadata_json: JSON.stringify({
      format: { format_name: 'mp4', duration: '5400.0' },
      streams: [
        { codec_type: 'video', codec_name: 'h264' },
        { codec_type: 'audio', codec_name: 'aac', tags: { language: 'jpn', title: 'Japanese' }, disposition: { default: 1 } },
        { codec_type: 'audio', codec_name: 'aac', tags: { language: 'eng', title: 'English' }, disposition: { default: 0 } },
      ],
    }, null, 2),
    metadata_updated_at: 1760923200,
    poster_url: '/api/v1/items/101/artwork?kind=poster',
    backdrop_url: '/api/v1/items/101/artwork?kind=backdrop',
    theme_song_url: '/api/v1/items/101/theme',
    tagline: 'Welcome to the real world.',
    overview: 'A computer hacker learns the true nature of reality and his role in the war against its controllers.',
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
    subtitle_tracks: [
      {
        index: 0,
        label: 'EN',
        format: 'SRT',
        url: '/api/v1/items/101/subtitles/0',
      },
    ],
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
    children: [
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
        genres: ['Drama', 'Fantasy'],
        modified_at: 1760923150,
      },
    ],
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
    hierarchy: [
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
        genres: ['Drama', 'Fantasy'],
        modified_at: 1760923150,
      },
    ],
    children: [
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
        genres: ['Drama', 'Fantasy'],
        modified_at: 1760923100,
      },
    ],
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
    hierarchy: [
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
        genres: ['Drama', 'Fantasy'],
        modified_at: 1760923150,
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
        genres: ['Drama', 'Fantasy'],
        modified_at: 1760923150,
      },
    ],
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
    description: 'Alternative movie and television metadata provider with strong series and episode coverage.',
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

const metadataSearchResults: Record<number, MetadataSearchResult[]> = {
  101: [
    {
      provider_id: 'tmdb',
      external_id: '603',
      media_type: 'movie',
      title: 'The Matrix',
      overview: 'A computer hacker learns the true nature of reality and his role in the war against its controllers.',
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
      overview: 'Nine noble families wage war against each other in order to gain control over the mythical land of Westeros.',
      artwork_url: 'https://image.tmdb.org/t/p/w500/u3bZgnGQ9T01sWNhyveQz0wH0Hl.jpg',
      backdrop_url: 'https://image.tmdb.org/t/p/w1280/suopoADq0k8YZr4dQXcU6pToj6s.jpg',
      release_year: 2011,
    },
  ],
};

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

function activeMockUserId(): number | undefined {
  const token = globalThis.localStorage.getItem(AUTH_TOKEN_STORAGE_KEY)?.trim();
  if (!token?.startsWith('mock-token-')) {
    return undefined;
  }

  const parsed = Number(token.slice('mock-token-'.length));
  return Number.isFinite(parsed) ? parsed : undefined;
}

const itemMetadata: Record<number, ItemMetadataResponse> = {
  101: {
    item_id: 101,
    providers: metadataProviders,
    matches: [
      {
        id: 1,
        provider_id: 'tmdb',
        external_id: '603',
        title: 'The Matrix',
        overview: 'A computer hacker learns the true nature of reality and his role in the war against its controllers.',
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
          { id: 1, person_id: 1, external_id: '6384', name: 'Keanu Reeves', role: 'Actor', department: 'Cast', character_name: 'Neo', image_url: 'https://image.tmdb.org/t/p/w185/4D0PpNI0kmP58hgrwGC3wCjxhnm.jpg', profile_url: 'https://www.themoviedb.org/person/6384', sort_order: 0 },
          { id: 2, person_id: 2, external_id: '2975', name: 'Laurence Fishburne', role: 'Actor', department: 'Cast', character_name: 'Morpheus', image_url: 'https://image.tmdb.org/t/p/w185/8suOhUmPbfKqDQ17jQ1Gy0mI3P4.jpg', profile_url: 'https://www.themoviedb.org/person/2975', sort_order: 1 },
          { id: 3, person_id: 3, external_id: '9340', name: 'Carrie-Anne Moss', role: 'Actor', department: 'Cast', character_name: 'Trinity', image_url: 'https://image.tmdb.org/t/p/w185/xD4jTA3KmVp5Rq3aHcymL9DUGjD.jpg', profile_url: 'https://www.themoviedb.org/person/9340', sort_order: 2 },
          { id: 4, person_id: 4, external_id: '9339', name: 'Lana Wachowski', role: 'Director', department: 'Directing', profile_url: 'https://www.themoviedb.org/person/9339', sort_order: 10000 },
          { id: 5, person_id: 5, external_id: '9341', name: 'Lilly Wachowski', role: 'Director', department: 'Directing', profile_url: 'https://www.themoviedb.org/person/9341', sort_order: 10001 },
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
  },
  201: {
    item_id: 201,
    providers: metadataProviders,
    matches: metadataMatchesWithSecondaries(
      items.find((item) => item.id === 201),
      {
        id: 3,
        provider_id: 'tmdb',
        external_id: '1399',
        title: 'Game of Thrones',
        overview: 'Nine noble families wage war against each other in order to gain control over the mythical land of Westeros.',
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
      },
    ),
  },
  202: {
    item_id: 202,
    providers: metadataProviders,
    matches: [],
  },
  203: {
    item_id: 203,
    providers: metadataProviders,
    matches: [],
  },
  103: {
    item_id: 103,
    providers: metadataProviders,
    matches: [],
  },
  104: {
    item_id: 104,
    providers: metadataProviders,
    matches: [],
  },
};

function themerrSecondaryMatch(
  item: MediaItemSummary,
  primaryMatch: ItemMetadataMatch,
  id: number,
): ItemMetadataMatch | undefined {
  if (item.item_type !== 'movie' && item.item_type !== 'show') {
    return undefined;
  }
  if (primaryMatch.provider_id !== 'tmdb') {
    return undefined;
  }

  return {
    id,
    provider_id: 'themerr',
    external_id: `${item.item_type}:tmdb:${primaryMatch.external_id}`,
    media_type: item.item_type,
    relation_kind: 'secondary',
    match_state: 'linked',
    theme_song_url: item.item_type === 'show'
      ? 'https://www.youtube.com/watch?v=uXZd_W5B7N0'
      : 'https://www.youtube.com/watch?v=SLBACEP6LsI',
    genres: [],
    people: [],
    locale_key: 'en-US',
    refresh_state: 'fresh',
    last_refreshed_at: primaryMatch.last_refreshed_at,
    updated_at: primaryMatch.updated_at,
  };
}

function metadataMatchesWithSecondaries(
  item: MediaItemSummary | undefined,
  primaryMatch: ItemMetadataMatch,
): ItemMetadataMatch[] {
  const secondaryMatch = item
    ? themerrSecondaryMatch(item, primaryMatch, primaryMatch.id + 1)
    : undefined;
  return secondaryMatch ? [primaryMatch, secondaryMatch] : [primaryMatch];
}

interface MockPlaybackProgress extends PlaybackProgressRequest {
  watch_count: number;
  last_watched_at?: number;
}

const playbackProgress = new Map<string, MockPlaybackProgress>();
playbackProgress.set('1:101', { position_ms: 1_260_000, duration_ms: 5_400_000, completed: false, watch_count: 0 });
playbackProgress.set('1:103', { position_ms: 74_000, duration_ms: 215_000, completed: false, watch_count: 0 });

function applyMockPlaybackProgress<T extends { id: number }>(item: T): T {
  const userId = activeMockUserId();
  const progress = userId === undefined ? undefined : playbackProgress.get(`${userId}:${item.id}`);
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

const collections: MediaCollectionSummary[] = [
  {
    id: 'tmdb:2344',
    provider_id: 'tmdb',
    external_id: '2344',
    name: 'The Matrix Collection',
    overview: 'A cyberpunk science-fiction collection centered around Neo, Zion, and the war against the machines.',
    artwork_url: 'https://image.tmdb.org/t/p/w500/f89U3ADr1oiB1s9GkdPOEpXUk5H.jpg',
    backdrop_url: 'https://image.tmdb.org/t/p/w1280/icmmSD4vTTDKOq2vvdulafOGw93.jpg',
    theme_song_url: 'https://www.youtube.com/watch?v=SLBACEP6LsI',
    item_ids: [101],
    item_count: 1,
  },
];

let settings: SettingsSnapshot = {
  general: {
    data_dir: 'C:/Users/Mock/AppData/Local/Koko/data',
  },
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
      weekdays: ['monday', 'tuesday', 'wednesday', 'thursday', 'friday', 'saturday', 'sunday'],
    },
    metadata_refresh: {
      enabled: true,
    },
    trash_cleanup: {
      enabled: false,
      missing_item_auto_delete_days: null,
      interval_days: 1,
    },
    database_maintenance: {
      enabled: true,
      interval_days: 7,
    },
  },
  server: {
    use_https: false,
    address: '127.0.0.1',
    port: 9191,
    cert_path: 'cert.pem',
    key_path: 'key.pem',
    use_custom_certs: false,
  },
  ffmpeg: {
    ffmpeg_path: 'ffmpeg',
    ffprobe_path: 'ffprobe',
  },
};

export function getMockCapabilities(): ServerCapabilities {
  return {
    app_name: 'Koko',
    version: '0.0.0-dev',
    server_url: 'http://127.0.0.1:9191',
    https_enabled: false,
    libraries_configured: libraries.length,
    api_versions: ['v1'],
    transcoding: {
      ffmpeg: {
        available: true,
        version: 'ffmpeg mock build',
      },
      ffprobe: {
        available: true,
        version: 'ffprobe mock build',
      },
    },
  };
}

export function getMockBootstrap(): AppBootstrapResponse {
  const currentUser = users.find((user) => user.id === activeMockUserId());
  return {
    has_users: users.length > 0,
    current_user: currentUser ? toUserSummary(currentUser) : undefined,
  };
}

export function loginMockUser(request: LoginRequest): TokenResponse {
  const user = users.find((candidate) => {
    return candidate.username === request.username && candidate.password === request.password;
  });
  if (!user) {
    throw new Error('401 Unauthorized');
  }
  return { token: `mock-token-${user.id}` };
}

export function createMockUser(request: CreateUserRequest): string {
  const currentUser = users.find((user) => user.id === activeMockUserId());
  if (users.length > 0 && currentUser === undefined) {
    throw new Error('401 Unauthorized');
  }

  if (users.length > 0 && !currentUser?.admin) {
    throw new Error('403 Forbidden');
  }

  if (users.some((user) => user.username.toLowerCase() === request.username.trim().toLowerCase())) {
    throw new Error('409 Conflict');
  }

  users.push({
    id: nextUserId,
    username: request.username.trim(),
    password: request.password,
    pin: request.pin,
    admin: users.length === 0 || request.admin,
    birthday: request.birthday?.trim() || undefined,
    profile_image_url: mockProfileImageUrl(request.profile_image_upload),
    preferred_metadata_languages: request.preferred_metadata_languages?.length
      ? request.preferred_metadata_languages
      : ['en-US'],
  });
  nextUserId += 1;
  return 'User created';
}

export function getMockUsers(): BootstrapUser[] {
  return users.map(toUserSummary);
}

export function updateMockUser(userId: number, request: UpdateUserRequest): BootstrapUser {
  const currentUser = users.find((user) => user.id === activeMockUserId());
  if (!currentUser) {
    throw new Error('401 Unauthorized');
  }
  if (!currentUser.admin) {
    throw new Error('403 Forbidden');
  }

  const user = users.find((candidate) => candidate.id === userId);
  if (!user) {
    throw new Error('404 Not Found');
  }

  const username = request.username.trim();
  if (!username) {
    throw new Error('400 Bad Request');
  }
  if (users.some((candidate) => candidate.id !== userId && candidate.username.toLowerCase() === username.toLowerCase())) {
    throw new Error('409 Conflict');
  }
  if (user.admin && !request.admin && users.filter((candidate) => candidate.admin).length <= 1) {
    throw new Error('400 Bad Request');
  }

  user.username = username;
  user.admin = request.admin;
  user.birthday = request.birthday?.trim() || undefined;
  const nextProfileImageUrl = mockProfileImageUrl(request.profile_image_upload);
  if (nextProfileImageUrl || request.remove_profile_image) {
    user.profile_image_url = nextProfileImageUrl;
  }
  user.preferred_metadata_languages = request.preferred_metadata_languages?.length
    ? request.preferred_metadata_languages
    : ['en-US'];
  return toUserSummary(user);
}

function toUserSummary(user: MockUserRecord): BootstrapUser {
  return {
    id: user.id,
    username: user.username,
    admin: user.admin,
    birthday: user.birthday,
    profile_image_url: user.profile_image_url,
    preferred_metadata_languages: user.preferred_metadata_languages ?? ['en-US'],
  };
}

export function getMockLibraries(): MediaLibrary[] {
  syncAllMockLibraryRefreshProgress();
  return [...libraries];
}

function syncMockLibraryRefreshProgress(libraryId: number): void {
  const library = libraries.find((candidate) => candidate.id === libraryId);
  if (!library) {
    return;
  }

  const refreshableItems = items.filter((item) => item.library_id === libraryId && item.has_metadata);
  library.metadata_refresh_total = refreshableItems.length;
  library.metadata_refresh_pending = refreshableItems.filter((item) => item.metadata_refresh_state === 'pending').length;
  library.metadata_refresh_failed = refreshableItems.filter((item) => item.metadata_refresh_state === 'error').length;
  library.metadata_refresh_completed = Math.max(0, library.metadata_refresh_total - library.metadata_refresh_pending);
}

function syncAllMockLibraryRefreshProgress(): void {
  libraries.forEach((library) => {
    syncMockLibraryRefreshProgress(library.id);
  });
}

export function getMockMetadataProviders(): MetadataProviderStatus[] {
  return metadataProviders.map((provider) => ({ ...provider }));
}

export function getMockSystemActivities(): SystemActivitiesResponse {
  const now = Math.floor(Date.now() / 1000);
  const activities = libraries.reduce<SystemActivity[]>((entries, library) => {
      const pendingItems = items.filter((item) => item.library_id === library.id && item.metadata_refresh_state === 'pending');
      if (!pendingItems.length) {
        return entries;
      }

      entries.push({
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

      return entries;
    }, []);

  return {
    generated_at: now,
    activities,
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
      message: 'Completed TMDB metadata refresh for media item 201 "Mock Show" (show) in library 2 [Mock Show]',
    },
    {
      timestamp: '2026-04-22T09:12:00.810-04:00',
      level: 'WARN',
      module: 'koko::web::routes::media',
      source_file_path: 'src/web/routes/media.rs',
      line_number: 589,
      message: 'Failed to fetch refreshed TMDB metadata snapshot for media item 417 "Season 1" (season) in library 2 [The Simpsons/Season 1] using target tv:456:season:1 (tv_season): TMDB season lookup failed with status 404 Not Found',
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
    const moduleMatches = moduleFilter ? entry.module.toLowerCase().includes(moduleFilter.toLowerCase()) : true;
    const searchMatches = search
      ? `${entry.message} ${entry.module} ${entry.source_file_path}`.toLowerCase().includes(search.toLowerCase())
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

export function getMockItem(itemId: number): MediaItemDetail | undefined {
  const item = items.find((item) => item.id === itemId);
  return item ? applyMockPlaybackProgress(item) : undefined;
}

export function getMockItemMetadata(itemId: number): ItemMetadataResponse | undefined {
  return itemMetadata[itemId];
}

export function searchMockItemMetadata(itemId: number, query?: string): MetadataSearchResult[] {
  const results = metadataSearchResults[itemId] ?? [];
  const normalized = query?.trim().toLowerCase();
  if (!normalized) {
    return [...results];
  }

  return results.filter((result) => {
    return result.title.toLowerCase().includes(normalized)
      || result.overview?.toLowerCase().includes(normalized);
  });
}

export function getMockPlayback(itemId: number): PlaybackDecision {
  const item = getMockItem(itemId);
  if (!item) {
    throw new Error('404 Not Found');
  }

  if (!item.playable) {
    return {
      item_id: itemId,
      can_direct_play: false,
      transcode_required: false,
      video_transcode_required: false,
      audio_transcode_required: false,
      reason: 'This item is a container and cannot be played directly.',
      stream_url: undefined,
      mime_type: undefined,
    };
  }

  const canDirectPlay = item.container === 'mp4' || item.container === 'mp3' || item.container === 'flac';
  return {
    item_id: itemId,
    can_direct_play: canDirectPlay,
    transcode_required: !canDirectPlay,
    video_transcode_required: !canDirectPlay && item.media_kind === 'video',
    audio_transcode_required: !canDirectPlay,
    reason: canDirectPlay
      ? 'Browser direct play is supported for this item.'
      : 'A future FFmpeg-backed transcode path will be required for browser playback.',
    stream_url: canDirectPlay ? `/api/v1/items/${itemId}/stream` : undefined,
    mime_type: item.media_kind === 'video' ? 'video/mp4' : 'audio/mpeg',
  };
}

export function getMockHome(libraryId?: number): MediaHome {
  const filteredItems = getMockItems(libraryId);
  const continueWatching = filteredItems.filter((item) => {
    const userId = activeMockUserId();
    const progress = userId === undefined ? undefined : playbackProgress.get(`${userId}:${item.id}`);
    return Boolean(progress && !progress.completed && progress.position_ms > 0);
  });
  const recentlyAdded = [...filteredItems].sort((left, right) => (right.modified_at ?? 0) - (left.modified_at ?? 0));
  const recommended = filteredItems.filter((item) => !continueWatching.some((candidate) => candidate.id === item.id));

  return {
    library_id: libraryId,
    shelves: [
      { id: 'continue_watching', title: 'Continue watching', items: continueWatching },
      { id: 'recently_added', title: 'Recently added', items: recentlyAdded },
      { id: 'recommended', title: 'Recommended', items: recommended },
    ],
    collections: collections.filter((collection) => collection.item_ids.some((itemId) => filteredItems.some((item) => item.id === itemId))),
  };
}

export function getMockItems(libraryId?: number): MediaItemSummary[] {
  return items
    .filter((item) => (typeof libraryId === 'number' ? item.library_id === libraryId : true))
    .map(({ file_size: _fileSize, container: _container, bit_rate: _bitRate, video_codec: _videoCodec, audio_codec: _audioCodec, metadata_json: _metadataJson, metadata_updated_at: _metadataUpdatedAt, ...summary }) => applyMockPlaybackProgress(summary));
}

export function searchMockItems(query: string): MediaSearchResult[] {
  const normalizedQuery = query.trim().toLowerCase();
  if (!normalizedQuery) {
    return [];
  }

  const results: MediaSearchResult[] = getMockItems().filter((item) => {
    return item.display_title.toLowerCase().includes(normalizedQuery)
      || item.relative_path.toLowerCase().includes(normalizedQuery)
      || item.media_kind.toLowerCase().includes(normalizedQuery);
  }).map((item) => ({ result_type: 'item', item }));

  results.push(...collections
    .filter((collection) => {
      return collection.name.toLowerCase().includes(normalizedQuery);
    })
    .map((collection) => ({ result_type: 'collection' as const, collection })));

  const people = new Map<number, MediaSearchResult>();
  for (const response of Object.values(itemMetadata)) {
    for (const match of response.matches) {
      for (const person of match.people) {
        if (
          person.name.toLowerCase().includes(normalizedQuery)
          || person.character_name?.toLowerCase().includes(normalizedQuery)
        ) {
          people.set(person.person_id, {
            result_type: 'person',
            person: {
              id: person.person_id,
              provider_id: match.provider_id,
              external_id: person.external_id,
              locale_key: person.locale_key ?? 'eng',
              name: person.name,
              known_for: [],
              profile_url: person.profile_url,
              image_url: person.image_url,
              cached_image_path: person.cached_image_path,
            },
          });
        }
      }
    }
  }

  results.push(...people.values());
  return results;
}

export function getMockSettings(): SettingsResponse {
  return {
    settings: structuredClone(settings),
    settings_path: 'C:/Users/Mock/AppData/Local/Koko/settings.yml',
  };
}

export function updateMockSettings(nextSettings: SettingsSnapshot): SettingsResponse {
  settings = structuredClone(nextSettings);
  return getMockSettings();
}

function mockProfileImageUrl(upload?: { mime_type: string; data_base64: string }): string | undefined {
  if (!upload?.data_base64) {
    return undefined;
  }
  return `data:${upload.mime_type};base64,${upload.data_base64}`;
}

export function clearMockMetadataCache(): { removed_files: number } {
  return { removed_files: 0 };
}

export function runMockScheduledTask(taskId: ScheduledTaskId): ScheduledTaskRunResponse {
  if (!['metadata_refresh', 'trash_cleanup', 'database_maintenance'].includes(taskId)) {
    throw new Error('404 Not Found');
  }

  return {
    task_id: taskId,
    started: true,
    message: `${taskId.replace(/_/g, ' ')} started`,
  };
}

export function addMockLibrary(request: { library: MediaLibrarySettings }): SettingsResponse {
  const normalizedLibrary = structuredClone(request.library);
  normalizedLibrary.paths = normalizedLibrary.paths.length ? normalizedLibrary.paths : [normalizedLibrary.path].filter(Boolean);
  normalizedLibrary.path = normalizedLibrary.paths[0] ?? normalizedLibrary.path;
  normalizedLibrary.metadata_languages = normalizedLibrary.metadata_languages?.length ? normalizedLibrary.metadata_languages : ['en-US'];
  normalizedLibrary.metadata_language_mode = normalizedLibrary.metadata_language_mode ?? 'auto';
  normalizedLibrary.scanner = normalizedLibrary.scanner ?? 'auto';
  normalizedLibrary.allowed_user_ids = normalizedLibrary.allowed_user_ids ?? [];
  settings.media.libraries.push(normalizedLibrary);
  libraries.push({
    id: nextLibraryId,
    name: normalizedLibrary.name,
    path: normalizedLibrary.path,
    paths: [...normalizedLibrary.paths],
    recursive: normalizedLibrary.recursive,
    kind: normalizedLibrary.kind,
    scanner: normalizedLibrary.scanner,
    metadata_providers: [...normalizedLibrary.metadata_providers],
    metadata_language_mode: normalizedLibrary.metadata_language_mode,
    metadata_languages: [...normalizedLibrary.metadata_languages],
    status: 'available',
    scan_revision: 1,
    last_scanned_at: Math.floor(Date.now() / 1000),
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
  nextLibraryId += 1;
  return getMockSettings();
}

export function removeMockLibrary(libraryIndex: number): SettingsResponse {
  if (libraryIndex < 0 || libraryIndex >= settings.media.libraries.length) {
    throw new Error('404 Not Found');
  }

  const [removedLibrary] = settings.media.libraries.splice(libraryIndex, 1);
  const libraryMatchIndex = libraries.findIndex((library) => {
    return library.name === removedLibrary.name && library.path === removedLibrary.path;
  });
  if (libraryMatchIndex >= 0) {
    libraries.splice(libraryMatchIndex, 1);
  }

  return getMockSettings();
}

export function deleteMockMissingItems(libraryId: number): MissingItemsCleanupResponse {
  const library = libraries.find((candidate) => candidate.id === libraryId);
  if (!library) {
    throw new Error('404 Not Found');
  }

  const deletedItems = items.filter((item) => item.library_id === libraryId && item.missing_since).length;
  for (let index = items.length - 1; index >= 0; index -= 1) {
    if (items[index].library_id === libraryId && items[index].missing_since) {
      items.splice(index, 1);
    }
  }

  collections.forEach((collection) => {
    collection.item_ids = collection.item_ids.filter((itemId) => items.some((item) => item.id === itemId));
    collection.item_count = collection.item_ids.length;
  });
  library.missing_files = 0;
  library.missing_items = 0;

  return {
    library_id: libraryId,
    deleted_files: deletedItems,
    deleted_items: deletedItems,
    removed_collection_items: 0,
    library: { ...library },
  };
}

export function updateMockPlaybackProgress(itemId: number, payload: PlaybackProgressRequest): void {
  const userId = activeMockUserId();
  if (userId !== undefined) {
    const key = `${userId}:${itemId}`;
    const existing = playbackProgress.get(key);
    const completedTransition = payload.completed && !existing?.completed;
    playbackProgress.set(key, {
      ...payload,
      watch_count: (existing?.watch_count ?? 0) + (completedTransition ? 1 : 0),
      last_watched_at: completedTransition
        ? Math.floor(Date.now() / 1000)
        : existing?.last_watched_at,
    });
  }
}

export function linkMockItemMetadata(itemId: number, request: LinkMetadataRequest): ItemMetadataMatch {
  const candidate = (metadataSearchResults[itemId] ?? []).find((result) => {
    return result.provider_id === request.provider_id
      && result.external_id === request.external_id
      && result.media_type === request.media_type;
  });
  if (!candidate) {
    throw new Error('404 Not Found');
  }

  const linkedMatch: ItemMetadataMatch = {
    id: Date.now(),
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
    updated_at: Math.floor(Date.now() / 1000),
  };

  const item = items.find((candidate) => candidate.id === itemId);

  itemMetadata[itemId] = {
    item_id: itemId,
    providers: metadataProviders,
    matches: metadataMatchesWithSecondaries(item, linkedMatch),
  };
  if (item) {
    item.display_title = candidate.title;
  }

  return linkedMatch;
}

function getMockPersonCreditsForMatch(
  itemId: number,
  item: MediaItemSummary,
  match: ItemMetadataMatch,
  personId: number,
): MetadataPersonItemCredit[] {
  return match.people
    .filter((person) => person.person_id === personId)
    .map((person) => mockPersonCredit(itemId, item, match, person));
}

function mockPersonCredit(
  itemId: number,
  item: MediaItemSummary,
  match: ItemMetadataMatch,
  person: ItemMetadataPerson,
): MetadataPersonItemCredit {
  return {
    credit: {
      id: person.id,
      metadata_link_id: match.id,
      media_item_id: itemId,
      role: person.role,
      department: person.department,
      character_name: person.character_name,
      sort_order: person.sort_order,
    },
    item,
    hierarchy: item.hierarchy ?? [],
  };
}

function getMockPersonCreditsForResponse(response: ItemMetadataResponse, personId: number): MetadataPersonItemCredit[] {
  const item = items.find((candidate) => candidate.id === response.item_id);
  if (!item) {
    return [];
  }

  return response.matches.flatMap((match) => getMockPersonCreditsForMatch(response.item_id, item, match, personId));
}

export function getMockPerson(personId: number): MetadataPersonResponse {
  const credits = Object.values(itemMetadata)
    .flatMap((response) => getMockPersonCreditsForResponse(response, personId));
  const firstCredit = credits[0];
  const personCredit = Object.values(itemMetadata)
    .flatMap((response) => response.matches)
    .flatMap((match) => match.people)
    .find((person) => person.person_id === personId);
  if (!firstCredit || !personCredit) {
    throw new Error('404 Not Found');
  }

  return {
    person: {
      id: personCredit.person_id,
      provider_id: 'tmdb',
      external_id: personCredit.external_id,
      locale_key: 'en-US',
      name: personCredit.name,
      known_for: ['The Matrix'],
      biography: personCredit.name === 'Keanu Reeves'
        ? 'Canadian actor known for action films, science fiction, and understated dramatic work.'
        : undefined,
      gender: personCredit.name === 'Carrie-Anne Moss' ? 'Female' : 'Male',
      birthday: personCredit.name === 'Keanu Reeves' ? '1964-09-02' : undefined,
      birth_place: personCredit.name === 'Keanu Reeves' ? 'Beirut, Lebanon' : undefined,
      profile_url: personCredit.profile_url,
      image_url: personCredit.image_url,
    },
    credits,
  };
}

export function refreshMockItemMetadata(itemId: number): ItemMetadataMatch {
  const response = itemMetadata[itemId];
  const existingMatch = response?.matches.find((match) => match.relation_kind === 'primary')
    ?? response?.matches[0];
  if (!existingMatch) {
    throw new Error('404 Not Found');
  }

  const pendingMatch: ItemMetadataMatch = {
    ...existingMatch,
    refresh_state: 'pending',
    updated_at: Math.floor(Date.now() / 1000),
  };

  itemMetadata[itemId] = {
    ...response,
    matches: response.matches.map((match) => match.id === existingMatch.id ? pendingMatch : match),
  };

  const item = items.find((candidate) => candidate.id === itemId);
  if (item) {
    item.metadata_refresh_state = 'pending';
    syncMockLibraryRefreshProgress(item.library_id);

    globalThis.setTimeout(() => {
      const source = (metadataSearchResults[itemId] ?? []).find((candidate) => {
        return candidate.provider_id === existingMatch.provider_id
          && candidate.external_id === existingMatch.external_id
          && candidate.media_type === existingMatch.media_type;
      });
      const refreshedAt = Math.floor(Date.now() / 1000);
      const refreshedMatch: ItemMetadataMatch = {
        ...pendingMatch,
        title: source?.title ?? existingMatch.title,
        overview: source?.overview ?? existingMatch.overview,
        artwork_url: source?.artwork_url ?? existingMatch.artwork_url,
        backdrop_url: source?.backdrop_url ?? existingMatch.backdrop_url,
        release_year: source?.release_year ?? existingMatch.release_year,
        refresh_state: 'fresh',
        updated_at: refreshedAt,
      };

      itemMetadata[itemId] = {
        ...response,
        matches: response.matches.map((match) => match.id === existingMatch.id ? refreshedMatch : match),
      };
      item.display_title = refreshedMatch.title ?? item.display_title;
      item.overview = refreshedMatch.overview ?? item.overview;
      item.release_year = refreshedMatch.release_year ?? item.release_year;
      item.linked_media_type = refreshedMatch.media_type ?? item.linked_media_type;
      item.metadata_refresh_state = 'fresh';
      item.artwork_updated_at = refreshedAt;
      syncMockLibraryRefreshProgress(item.library_id);
    }, 900);
  }

  return pendingMatch;
}

export function refreshMockLibraryMetadata(libraryId: number): MediaLibrary {
  const library = libraries.find((candidate) => candidate.id === libraryId);
  if (!library) {
    throw new Error('404 Not Found');
  }

  const refreshableItems = items.filter((item) => item.library_id === libraryId && item.has_metadata);
  refreshableItems.forEach((item) => {
    item.metadata_refresh_state = 'pending';
    const response = itemMetadata[item.id];
    if (response) {
      itemMetadata[item.id] = {
        ...response,
        matches: response.matches.map((match) => match.relation_kind === 'primary'
          ? {
              ...match,
              refresh_state: 'pending',
              updated_at: Math.floor(Date.now() / 1000),
            }
          : match),
      };
    }
  });
  syncMockLibraryRefreshProgress(libraryId);

  globalThis.setTimeout(() => {
    const refreshedAt = Math.floor(Date.now() / 1000);
    refreshableItems.forEach((item) => {
      item.metadata_refresh_state = 'fresh';
      item.artwork_updated_at = refreshedAt;
      const response = itemMetadata[item.id];
      if (response) {
        itemMetadata[item.id] = {
          ...response,
          matches: response.matches.map((match) => match.relation_kind === 'primary'
            ? {
                ...match,
                refresh_state: 'fresh',
                updated_at: refreshedAt,
              }
            : match),
        };
      }
    });
    syncMockLibraryRefreshProgress(libraryId);
  }, 1200);

  return { ...library };
}
