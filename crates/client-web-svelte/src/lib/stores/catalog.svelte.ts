// Catalog store — home, items, search, browse tabs/filters, and the home
// feature preview selection. Replaces state.home / state.libraryItems /
// state.searchResults / state.homeTab / state.browseFilter and the
// homeFeaturePreview / browseItemsForRoute selectors.
import {
  getHome,
  getItems,
  searchItems,
  type MediaHome,
  type MediaItemSummary,
  type MediaSearchResult,
} from '$lib/api';

export type HomeTab = 'recommended' | 'library' | 'collections' | 'playlists' | 'categories';
export type BrowseFilter =
  | { kind: 'category'; key: string }
  | { kind: 'collection'; key: string }
  | { kind: 'playlist'; key: string };

class CatalogStore {
  home = $state<MediaHome | undefined>(undefined);
  libraryItems = $state<MediaItemSummary[]>([]);
  libraryItemsLoading = $state(false);
  searchQuery = $state('');
  searchResults = $state<MediaSearchResult[]>([]);
  homeTab = $state<HomeTab>('recommended');
  browseFilter = $state<BrowseFilter | undefined>(undefined);
  activeLibraryId = $state<number | undefined>(undefined);

  // Preview state — which item/collection the home hero shows.
  homePreviewItemId = $state<number | undefined>(undefined);
  homePreviewCollectionId = $state<string | undefined>(undefined);

  async loadHome(libraryId?: number) {
    this.activeLibraryId = libraryId;
    this.home = await getHome(libraryId);
  }

  async loadLibraryItems(libraryId?: number) {
    this.libraryItemsLoading = true;
    try {
      this.libraryItems = await getItems(libraryId);
    } finally {
      this.libraryItemsLoading = false;
    }
  }

  async runSearch(query: string) {
    this.searchQuery = query;
    if (!query.trim()) {
      this.searchResults = [];
      return;
    }
    this.searchResults = await searchItems(query);
  }

  clearSearch() {
    this.searchQuery = '';
    this.searchResults = [];
  }

  resetForRouteChange() {
    this.homeTab = 'recommended';
    this.browseFilter = undefined;
    this.clearSearch();
  }
}

export const catalog = new CatalogStore();
