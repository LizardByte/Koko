// Item store — selectedItem / selectedItemMetadata / selectedPerson /
// selectedPlayback (state.selected* in the vanilla client).
import {
  getItem,
  getItemMetadata,
  getPerson,
  getPlaybackDecision,
  searchItemMetadata,
  linkItemMetadata,
  refreshItemMetadata,
  type MediaItemDetail,
  type ItemMetadataResponse,
  type MetadataPersonResponse,
  type PlaybackDecision,
  type MetadataSearchResult,
  type MetadataSearchOptions,
  type LinkMetadataRequest,
} from '$lib/api';

class ItemStore {
  item = $state<MediaItemDetail | undefined>(undefined);
  metadata = $state<ItemMetadataResponse | undefined>(undefined);
  person = $state<MetadataPersonResponse | undefined>(undefined);
  playback = $state<PlaybackDecision | undefined>(undefined);
  loading = $state(false);

  // Metadata search/link state (mirrors vanilla state.metadataSearch*).
  metadataSearchResults = $state<MetadataSearchResult[]>([]);
  metadataSearching = $state(false);

  async loadItem(itemId: number) {
    this.loading = true;
    try {
      const [item, metadata] = await Promise.all([
        getItem(itemId),
        getItemMetadata(itemId).catch(() => undefined),
      ]);
      this.item = item;
      this.metadata = metadata;
      if (item.playable) {
        this.playback = await getPlaybackDecision(itemId).catch(() => undefined);
      } else {
        this.playback = undefined;
      }
    } finally {
      this.loading = false;
    }
  }

  async loadPerson(personId: number) {
    this.loading = true;
    try {
      this.person = await getPerson(personId);
    } finally {
      this.loading = false;
    }
  }

  /**
   * Search metadata providers for manual linking. Mirrors vanilla
   * eventBindings.ts:737-766 (the #metadata-search-form submit handler).
   * Results are stored in metadataSearchResults for the panel to render.
   */
  async searchMetadata(itemId: number, options?: MetadataSearchOptions | string) {
    this.metadataSearching = true;
    try {
      this.metadataSearchResults = await searchItemMetadata(itemId, options);
    } finally {
      this.metadataSearching = false;
    }
  }

  /**
   * Link a specific provider match to the item, then re-fetch metadata +
   * clear search results. Mirrors vanilla eventBindings.ts:768-790
   * (the [data-link-metadata] click handler).
   */
  async linkMetadata(itemId: number, request: LinkMetadataRequest) {
    await linkItemMetadata(itemId, request);
    this.metadata = await getItemMetadata(itemId);
    this.metadataSearchResults = [];
  }

  /**
   * Force-refresh the item's linked metadata, then re-fetch. Mirrors vanilla
   * eventBindings.ts:792-805 (the #refresh-item-metadata click handler).
   */
  async refreshMetadata(itemId: number) {
    await refreshItemMetadata(itemId);
    this.metadata = await getItemMetadata(itemId);
  }

  clear() {
    this.item = undefined;
    this.metadata = undefined;
    this.playback = undefined;
    this.metadataSearchResults = [];
  }

  clearPerson() {
    this.person = undefined;
  }
}

export const item = new ItemStore();
