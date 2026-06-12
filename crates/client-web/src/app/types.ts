import type {
  ApiMode,
  AppBootstrapResponse,
  BootstrapUser,
  ItemMetadataResponse,
  LogEntriesResponse,
  MediaHome,
  MediaItemDetail,
  MediaItemSummary,
  MediaLibrary,
  MediaSearchResult,
  MetadataPersonResponse,
  MetadataProviderStatus,
  MetadataSearchResult,
  PlaybackDecision,
  PlaybackSession,
  ServerCapabilities,
  SettingsResponse,
  SystemActivity,
} from '../api';

/** Client-side routes supported by the single-page web UI. */
export type AppRoute =
  | { page: 'home'; libraryId?: number }
  | { page: 'browse-detail'; kind: 'category' | 'collection' | 'playlist'; key: string; libraryId?: number }
  | { page: 'item'; itemId: number }
  | { page: 'person'; personId: number }
  | { page: 'settings'; section?: SettingsSection };

/** Top-level tabs shown on the home browsing surface. */
export type HomeBrowseTab = 'recommended' | 'library' | 'collections' | 'playlists' | 'categories';

/** Settings subsections addressable from navigation and direct URLs. */
export type SettingsSection = 'general' | 'libraries' | 'providers' | 'scheduled' | 'dashboard' | 'logs';

/** Describes a resolved browse filter used by category, collection, and playlist detail pages. */
export interface BrowseFilter {
  kind: 'category' | 'collection' | 'playlist';
  label: string;
  itemIds: number[];
  overview?: string;
  artworkUrl?: string;
}

/** Playback-ready trailer metadata used by the overlay player. */
export interface TrailerOption {
  title: string;
  url: string;
  label?: string;
  titleSuffix?: string;
}

/** Theme-song source variants supported by the web UI. */
export type ThemeSongSource =
  | { kind: 'audio'; src: string; title: string }
  | { kind: 'youtube'; src: string; title: string; videoId: string };

/** Minimal YouTube iframe player contract used by the client. */
export interface YouTubePlayer {
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

/** Browser global exposed after loading the YouTube iframe API script. */
export interface YouTubeIframeApi {
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

/** Mutable state for the browser client between render passes. */
export interface AppState {
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
  searchResults: MediaSearchResult[];
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
  activePlaybackItem?: MediaItemDetail;
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

/** Lucide icon names intentionally allowed by the web UI renderer. */
export type AppIconName =
  | 'arrow-left'
  | 'arrow-right'
  | 'book'
  | 'chevron-left'
  | 'chevron-right'
  | 'circle-check'
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

/** Grouped credit data used by the person detail renderer. */
export interface PersonSeasonCreditGroup {
  season: MediaItemSummary;
  episodes: MediaItemSummary[];
}

/** Top-level person credit group keyed by root media item. */
export interface PersonCreditGroup {
  root: MediaItemSummary;
  seasons: PersonSeasonCreditGroup[];
}
