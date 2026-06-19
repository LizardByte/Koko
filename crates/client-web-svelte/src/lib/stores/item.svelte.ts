// Item store — selectedItem / selectedItemMetadata / selectedPerson /
// selectedPlayback (state.selected* in the vanilla client).
import {
  getItem,
  getItemMetadata,
  getPerson,
  getPlaybackDecision,
  type MediaItemDetail,
  type ItemMetadataResponse,
  type MetadataPersonResponse,
  type PlaybackDecision,
} from '$lib/api';

class ItemStore {
  item = $state<MediaItemDetail | undefined>(undefined);
  metadata = $state<ItemMetadataResponse | undefined>(undefined);
  person = $state<MetadataPersonResponse | undefined>(undefined);
  playback = $state<PlaybackDecision | undefined>(undefined);
  loading = $state(false);

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

  clear() {
    this.item = undefined;
    this.metadata = undefined;
    this.playback = undefined;
  }

  clearPerson() {
    this.person = undefined;
  }
}

export const item = new ItemStore();
