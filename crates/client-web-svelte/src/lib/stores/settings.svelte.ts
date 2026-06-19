// Settings store — state.settingsResponse plus library/provider mutations.
import {
  getSettings,
  updateSettings,
  addLibrary,
  deleteLibrary,
  clearMetadataCache,
  type SettingsResponse,
  type SettingsSnapshot,
  type MediaLibrarySettings,
} from '$lib/api';

class SettingsStore {
  response = $state<SettingsResponse | undefined>(undefined);
  loading = $state(false);

  get settings(): SettingsSnapshot | undefined {
    return this.response?.settings;
  }

  get settingsPath(): string {
    return this.response?.settings_path ?? '';
  }

  async load() {
    this.loading = true;
    try {
      this.response = await getSettings();
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
