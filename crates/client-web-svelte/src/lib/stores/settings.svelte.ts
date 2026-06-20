// Settings store — state.settingsResponse plus library/provider mutations.
import {
  getSettings,
  updateSettings,
  addLibrary,
  deleteLibrary,
  clearMetadataCache,
  getMetadataProviders,
  type SettingsResponse,
  type SettingsSnapshot,
  type MediaLibrarySettings,
  type MetadataProviderStatus,
} from '$lib/api';

class SettingsStore {
  response = $state<SettingsResponse | undefined>(undefined);
  loading = $state(false);
  metadataProviders = $state<MetadataProviderStatus[]>([]);

  get settings(): SettingsSnapshot | undefined {
    return this.response?.settings;
  }

  get settingsPath(): string {
    return this.response?.settings_path ?? '';
  }

  async load() {
    this.loading = true;
    try {
      const [response, providers] = await Promise.all([
        getSettings(),
        getMetadataProviders().catch(() => []),
      ]);
      this.response = response;
      this.metadataProviders = providers;
    } finally {
      this.loading = false;
    }
  }

  async save(next: SettingsSnapshot) {
    this.response = await updateSettings(next);
  }

  async addLibrary(library: MediaLibrarySettings) {
    this.response = await addLibrary(library);
  }

  async deleteLibrary(index: number) {
    this.response = await deleteLibrary(index);
  }

  async clearMetadataCache() {
    await clearMetadataCache();
  }
}

export const settings = new SettingsStore();
