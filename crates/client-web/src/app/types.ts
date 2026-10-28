/** Defines browser routes, application state, and shared view contracts. */

import type {
  ApiMode,
  AudiobookRecordingChoices,
  AppBootstrapResponse,
  BootstrapUser,
  ItemMetadataResponse,
  LogEntriesResponse,
  MediaHome,
  MusicPlaylistDetail,
  MediaItemDetail,
  MediaItemSources,
  MediaItemSummary,
  MediaLibrary,
  MediaSearchResult,
  MetadataPersonResponse,
  MetadataPersonItemCredit,
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
  | { page: 'podcasts' }
  | { page: 'settings'; section?: SettingsSection };

/** Top-level tabs shown on the home browsing surface. */
export type HomeBrowseTab = 'recommended' | 'library' | 'collections' | 'playlists' | 'categories';

/** Settings subsections addressable from navigation and direct URLs. */
export type SettingsSection = 'general' | 'libraries' | 'providers' | 'scheduled' | 'users' | 'dashboard' | 'logs';

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
  var YT: YouTubeIframeApi | undefined;
  var onYouTubeIframeAPIReady: (() => void) | undefined;
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
  /** Discovery and subscription data belong to the account that requested them. */
  podcasts?: { owner: number; items: MediaItemSummary[]; directory: import('../api').DirectoryPodcast[]; country: string; query: string; home?: MediaHome; tab?: 'recommended' | 'library' | 'discover'; addFeedOpen?: boolean; loading: boolean; error?: string; busy?: boolean };
  /** In-flight season paging is scoped to the selected item and account. */
  podcastEpisodeLoading?: { itemId: number; owner: number; loading: boolean; error?: string };
  playlists?: MusicPlaylistDetail[];
  selectedPlaylist?: MusicPlaylistDetail;
  libraryItems: MediaItemSummary[];
  libraryItemsLoading: boolean;
  searchResults: MediaSearchResult[];
  homePreviewItemId?: number;
  homePreviewCollectionId?: string;
  metadataProviders: MetadataProviderStatus[];
  systemActivities: SystemActivity[];
  dashboardItems: MediaItemSummary[];
  settingsResponse?: SettingsResponse;
  settingsCreation?: 'libraries' | 'users';
  logsResponse?: LogEntriesResponse;
  logsLoading?: boolean;
  logsError?: string;
  logPage: number;
  selectedItem?: MediaItemDetail;
  /** Same-type recommendations, scoped to the item and account that requested them. */
  similarItems?: { itemId: number; userId?: number; items: MediaItemSummary[] };
  selectedItemMetadata?: ItemMetadataResponse;
  itemMetadataLoading: boolean;
  selectedPerson?: MetadataPersonResponse;
  itemCredits?: { itemId: number; loading: boolean; credits: MetadataPersonItemCredit[]; error?: string };
  selectedPlayback?: PlaybackDecision;
  metadataSearchResults: MetadataSearchResult[];
  /** Transient choices scoped to one item so late replies cannot populate another page. */
  audiobookRecordingChoices?: AudiobookRecordingChoices;
  searchQuery: string;
  metadataSearchQuery: string;
  metadataSearchArtist?: string;
  metadataSearchYear: string;
  metadataSearchLanguage: string;
  metadataSearchProviders: string[];
  showFullSearchResults: boolean;
  homeTab: HomeBrowseTab;
  browsePreferences: import('./browsePreferences').BrowsePreferences;
  browseFiltersOpen: boolean;
  libraryPage: number;
  itemManagementOpen: boolean;
  itemSources?: { itemId: number; userId?: number; loading: boolean; data?: MediaItemSources; error?: string };
  itemMoreOpen: boolean;
  resumeOptionsOpen: boolean;
  expandedCreditId?: number;
  expandedSeasonCreditId?: number;
  searchLoading: boolean;
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
  | 'highlighter'
  | 'notebook-pen'
  | 'square-pen'
  | 'pencil'
  | 'a-large-small'
  | 'bookmark'
  | 'bookmark-plus'
  | 'book-open'
  | 'columns-2'
  | 'list-tree'
  | 'minus'
  | 'minimize'
  | 'locate-fixed'
  | 'arrow-left'
  | 'arrow-right'
  | 'book'
  | 'chevron-down'
  | 'chevron-left'
  | 'chevron-right'
  | 'chevron-up'
  | 'check'
  | 'circle-check'
  | 'clapperboard'
  | 'database-zap'
  | 'ellipsis'
  | 'film'
  | 'folder-open'
  | 'folder-sync'
  | 'house'
  | 'image'
  | 'languages'
  | 'layers'
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
  | 'repeat'
  | 'repeat-1'
  | 'shuffle'
  | 'list-music'
  | 'download'
  | 'hard-drive-download'
  | 'cloud-download'
  | 'mic-2'
  | 'radio'
  | 'rotate-ccw'
  | 'save'
  | 'search'
  | 'settings'
  | 'skip-back'
  | 'skip-forward'
  | 'sliders-horizontal'
  | 'star'
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
