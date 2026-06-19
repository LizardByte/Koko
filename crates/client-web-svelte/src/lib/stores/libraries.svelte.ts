// Libraries store — holds the runtime library list (state.libraries in the
// vanilla client) and exposes load/refresh helpers.
import {
  getLibraries,
  scanLibrary,
  refreshLibraryMetadata,
  type MediaLibrary,
} from '$lib/api';

class LibrariesStore {
  libraries = $state<MediaLibrary[]>([]);
  loading = $state(false);

  async load() {
    this.loading = true;
    try {
      this.libraries = await getLibraries();
    } finally {
      this.loading = false;
    }
  }

  byId(id: number): MediaLibrary | undefined {
    return this.libraries.find((library) => library.id === id);
  }

  async scan(id: number) {
    const updated = await scanLibrary(id);
    this.replace(updated);
  }

  async refreshMetadata(id: number) {
    const updated = await refreshLibraryMetadata(id);
    this.replace(updated);
  }

  private replace(updated: MediaLibrary) {
    const index = this.libraries.findIndex((library) => library.id === updated.id);
    if (index >= 0) {
      this.libraries[index] = updated;
      this.libraries = [...this.libraries];
    }
  }
}

export const libraries = new LibrariesStore();
