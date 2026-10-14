import type { MediaItemSummary } from '../api';

export interface BrowsePreferences {
  sort: 'title' | 'recent' | 'year';
  watched: 'all' | 'unwatched' | 'in-progress' | 'watched';
  genre: string;
  year: string;
  density: 'comfortable' | 'compact' | 'spacious';
}

export const DEFAULT_BROWSE_PREFERENCES: BrowsePreferences = {
  sort: 'title', watched: 'all', genre: '', year: '', density: 'comfortable',
};

/** Sort and card size affect presentation, rather than narrowing the results. */
export function activeBrowseFilterCount(preferences: BrowsePreferences): number {
  return Number(Boolean(preferences.genre)) + Number(Boolean(preferences.year)) + Number(preferences.watched !== 'all');
}

/** Validate stored preferences so old or damaged browser storage cannot break browsing. */
export function normalizeBrowsePreferences(value: Partial<BrowsePreferences> | null): BrowsePreferences {
  return {
    sort: ['title', 'recent', 'year'].includes(value?.sort ?? '') ? value!.sort! : 'title',
    watched: ['all', 'unwatched', 'in-progress', 'watched'].includes(value?.watched ?? '') ? value!.watched! : 'all',
    genre: typeof value?.genre === 'string' ? value.genre : '',
    year: typeof value?.year === 'string' && /^\d{4}$/.test(value.year) ? value.year : '',
    density: ['comfortable', 'compact', 'spacious'].includes(value?.density ?? '') ? value!.density! : 'comfortable',
  };
}

function preferencesKey(userId: number | undefined, libraryId: number | undefined): string {
  return `koko:browse:${userId ?? 'guest'}:${libraryId ?? 'all'}`;
}

export function loadBrowsePreferences(userId?: number, libraryId?: number): BrowsePreferences {
  try {
    return normalizeBrowsePreferences(JSON.parse(localStorage.getItem(preferencesKey(userId, libraryId)) ?? 'null'));
  } catch {
    return { ...DEFAULT_BROWSE_PREFERENCES };
  }
}

export function saveBrowsePreferences(preferences: BrowsePreferences, userId?: number, libraryId?: number): void {
  try {
    localStorage.setItem(preferencesKey(userId, libraryId), JSON.stringify(preferences));
  } catch {
    // Browsing remains usable when storage is unavailable or full.
  }
}

export function browseWatchState(item: MediaItemSummary): 'unwatched' | 'in-progress' | 'watched' {
  if (item.playback_completed) return 'watched';
  if ((item.playback_position_ms ?? 0) >= 30_000) return 'in-progress';
  return (item.watch_count ?? 0) > 0 ? 'watched' : 'unwatched';
}

/** Filter before sorting, with deterministic title ordering for equal or missing values. */
export function browseItems(items: MediaItemSummary[], preferences: BrowsePreferences): MediaItemSummary[] {
  return items.filter((item) => (
    (!preferences.genre || item.genres.includes(preferences.genre))
    && (!preferences.year || String(item.release_year) === preferences.year)
    && (preferences.watched === 'all' || browseWatchState(item) === preferences.watched)
  )).sort((left, right) => {
    let order = 0;
    if (preferences.sort === 'recent') order = (right.modified_at ?? 0) - (left.modified_at ?? 0);
    if (preferences.sort === 'year') order = (right.release_year ?? 0) - (left.release_year ?? 0);
    return order || left.display_title.localeCompare(right.display_title, undefined, { numeric: true, sensitivity: 'base' }) || left.id - right.id;
  });
}
