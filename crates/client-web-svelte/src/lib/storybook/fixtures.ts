// Storybook fixtures — self-contained seed data builders for stories.
// Deliberately separate from mockApi.ts so stories don't pull the whole mock
// dispatch layer; these are plain data builders for the store singletons.
//
// Importing the store singletons and mutating their $state fields is the
// seeding mechanism (see presets.ts / WithStores decorator).

import type {
  BootstrapUser,
  ItemMetadataResponse,
  MediaCollectionSummary,
  MediaHome,
  MediaItemDetail,
  MediaItemExtra,
  MediaItemSummary,
  MediaLibrary,
} from '$lib/api';

// --- Items (MediaItemSummary shape — what cards/shelves render) ---

export function movieSummary(overrides: Partial<MediaItemSummary> = {}): MediaItemSummary {
  return {
    id: 101,
    library_id: 1,
    item_type: 'movie',
    display_title: 'Mock Movie',
    relative_path: 'Action/mock-movie.mp4',
    media_kind: 'video',
    playable: true,
    child_count: 0,
    duration_ms: 5_400_000,
    modified_at: 1_760_923_200,
    genres: ['Action', 'Sci-Fi'],
    has_metadata: true,
    metadata_refresh_state: 'fresh',
    ...overrides,
  };
}

export function showSummary(overrides: Partial<MediaItemSummary> = {}): MediaItemSummary {
  return {
    id: 201,
    library_id: 2,
    item_type: 'show',
    display_title: 'Mock Show',
    relative_path: 'Mock Show',
    media_kind: 'video',
    playable: false,
    child_count: 1,
    available_season_count: 1,
    duration_ms: 2_700_000,
    modified_at: 1_760_923_150,
    genres: ['Drama', 'Fantasy'],
    has_metadata: true,
    metadata_refresh_state: 'fresh',
    ...overrides,
  };
}

export function seasonSummary(overrides: Partial<MediaItemSummary> = {}): MediaItemSummary {
  return {
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
    modified_at: 1_760_923_150,
    genres: ['Drama', 'Fantasy'],
    has_metadata: true,
    metadata_refresh_state: 'fresh',
    ...overrides,
  };
}

export function episodeSummary(overrides: Partial<MediaItemSummary> = {}): MediaItemSummary {
  return {
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
    modified_at: 1_760_923_100,
    genres: ['Drama', 'Fantasy'],
    has_metadata: true,
    metadata_refresh_state: 'fresh',
    ...overrides,
  };
}

export function trackSummary(overrides: Partial<MediaItemSummary> = {}): MediaItemSummary {
  return {
    id: 103,
    library_id: 3,
    item_type: 'track',
    display_title: 'Mock Song',
    relative_path: 'mock-artist/mock-song.flac',
    media_kind: 'audio',
    playable: true,
    child_count: 0,
    duration_ms: 215_000,
    modified_at: 1_760_923_000,
    genres: [],
    // NOTE: has_metadata intentionally absent — exercises the unmatched badge
    // (has_metadata !== true). See MediaCard stories + the isUnmatched fix.
    ...overrides,
  };
}

// --- Item detail (for SectionHero/SectionSupport stories) ---

export function movieDetail(overrides: Partial<MediaItemDetail> = {}): MediaItemDetail {
  return {
    ...movieSummary(),
    file_size: 1_610_612_736,
    container: 'mp4',
    bit_rate: 2_400_000,
    video_codec: 'h264',
    audio_codec: 'aac',
    width: 1920,
    height: 1080,
    metadata_json: '{}',
    metadata_updated_at: 1_760_923_200,
    overview: 'A mock movie used to exercise the item detail view in stories.',
    tagline: 'Welcome to the real world.',
    release_year: 1999,
    content_rating: 'R',
    rating: 8.7,
    extras: [],
    audio_tracks: [],
    subtitle_tracks: [],
    hierarchy: [],
    children: [],
    ...overrides,
  } as MediaItemDetail;
}

// --- Libraries ---

export function mockLibraries(): MediaLibrary[] {
  return [
    {
      id: 1,
      name: 'Movies',
      path: 'C:/Media/Movies',
      paths: ['C:/Media/Movies'],
      recursive: true,
      kind: 'movies',
      scanner: 'movies',
      metadata_providers: ['tmdb'],
      metadata_language_mode: 'auto',
      metadata_languages: ['en-US'],
      status: 'available',
      scan_revision: 6,
      last_scanned_at: 1_760_923_200,
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
      last_scanned_at: 1_760_923_150,
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
      last_scanned_at: 1_760_923_100,
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
}

// --- Home (shelves + collections) ---

export function mockHome(): MediaHome {
  const movie = movieSummary();
  const show = showSummary();
  const season = seasonSummary();
  const episode = episodeSummary();
  const track = trackSummary();
  return {
    library_id: undefined,
    shelves: [
      {
        id: 'continue_watching',
        title: 'Continue watching',
        // Distinct id + title so this in-progress movie doesn't collide with
        // the canonical movie (id 101) in 'recently_added' when shelves are
        // flattened into libraryItems for BrowseListing resolution.
        items: [{ ...movie, id: 401, display_title: 'Mock Movie (continued)', playback_position_ms: 1_260_000, playback_completed: false }],
      },
      {
        id: 'recently_added',
        title: 'Recently added',
        items: [movie, show, season, episode, track, { ...movie, id: 301 }],
      },
      {
        id: 'recommended',
        title: 'Recommended',
        items: [show, season, episode],
      },
    ],
    collections: [
      {
        id: 'mock-collection',
        provider_id: 'tmdb',
        external_id: 'mock-collection',
        name: 'Mock Collection',
        item_ids: [101, 201],
        item_count: 2,
        overview: 'A mock collection for stories.',
      } as MediaCollectionSummary,
    ],
  };
}

// --- Auth ---

export function mockUser(overrides: Partial<BootstrapUser> = {}): BootstrapUser {
  return {
    id: 1,
    username: 'admin',
    admin: true,
    preferred_metadata_languages: ['en-US'],
    ...overrides,
  };
}

// --- Extras + metadata (for SectionExtras/SectionPeople/SectionSupport stories) ---

export function mockExtras(): MediaItemExtra[] {
  return [
    {
      extra_type: 'trailer',
      title: 'Official Trailer',
      url: 'https://example.com/trailer.mp4',
      duration_seconds: 132,
      thumbnail_url: 'https://example.com/trailer-thumb.jpg',
    },
    {
      extra_type: 'theme_song',
      title: 'Main Theme',
      url: 'https://www.youtube.com/watch?v=mock',
      duration_seconds: 90,
    },
  ];
}

export function mockMetadata(): ItemMetadataResponse {
  return {
    item_id: 101,
    providers: [
      {
        id: 'tmdb',
        display_name: 'TMDB',
        description: 'The Movie Database',
        supported_kinds: ['movies', 'shows'],
        requires_api_key: true,
        implemented: true,
        role: 'primary',
        extends_provider_ids: [],
        enabled: true,
        configured: true,
        language: 'en-US',
        attribution_text: 'TMDB',
        attribution_url: 'https://www.themoviedb.org',
      },
    ],
    matches: [
      {
        id: 101,
        provider_id: 'tmdb',
        external_id: '603',
        media_type: 'movie',
        title: 'Mock Movie',
        relation_kind: 'primary',
        match_state: 'linked',
        release_year: 1999,
        overview: 'A mock movie overview from the linked metadata provider.',
        genres: ['Action', 'Sci-Fi'],
        // Populated so SectionPeople renders real cast cards. No images wired
        // (cached_image_path / image_url absent) → cards show the initials
        // placeholder, exercising the no-photo branch.
        people: [
          { id: 1, person_id: 501, name: 'Ada Lovelace', character_name: 'The Architect', sort_order: 0 },
          { id: 2, person_id: 502, name: 'Alan Turing', character_name: 'The Cryptographer', sort_order: 1 },
          { id: 3, person_id: 503, name: 'Grace Hopper', character_name: 'The Admiral', sort_order: 2 },
          { id: 4, person_id: 504, name: 'Claude Shannon', role: 'Director', department: 'Directing', sort_order: 3 },
        ],
        locale_key: 'en-US',
      },
    ],
  };
}
