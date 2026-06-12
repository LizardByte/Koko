/** Derives active background activity and refresh progress state. */
import type { MediaItemSummary, MediaLibrary, SystemActivity } from '../api';
import { state } from './state';

export function activeMetadataRefreshActivities(): SystemActivity[] {
  return state.systemActivities.filter((activity) => {
    return activity.category === 'metadata_refresh'
      && activity.state !== 'completed'
      && activity.state !== 'failed';
  });
}

export function activeLibraryScanActivities(): SystemActivity[] {
  return state.systemActivities.filter((activity) => {
    return activity.category === 'library_scan'
      && activity.state !== 'completed'
      && activity.state !== 'failed';
  });
}

export function hasActiveLibraryScan(libraryId?: number): boolean {
  const activities = activeLibraryScanActivities();
  return libraryId === undefined
    ? activities.length > 0
    : activities.some((activity) => activity.library_id === libraryId);
}

export function activeMetadataRefreshItemIds(): Set<number> {
  return new Set(activeMetadataRefreshActivities().flatMap((activity) => activity.item_ids));
}

/** Returns whether an item is currently marked for metadata refresh. */
export function itemIsMetadataPending(item: Pick<MediaItemSummary, 'id' | 'metadata_refresh_state'> | undefined): boolean {
  return item?.metadata_refresh_state === 'pending';
}

export function itemHasActiveMetadataRefresh(item: Pick<MediaItemSummary, 'id' | 'metadata_refresh_state'> | undefined): boolean {
  return item?.metadata_refresh_state === 'pending' && activeMetadataRefreshItemIds().has(item.id);
}

/** Calculates stored metadata refresh progress from a library summary. */
export function libraryRefreshProgress(library: MediaLibrary): { completed: number; total: number; percent: number; failed: number } | undefined {
  const activityProgress = metadataRefreshActivityProgressForLibrary(library.id);
  if (activityProgress) {
    return activityProgress;
  }

  if (library.metadata_refresh_total <= 0 || library.metadata_refresh_pending <= 0) {
    return undefined;
  }

  const completed = Math.max(0, library.metadata_refresh_completed);
  const percent = Math.min(100, Math.max(0, (completed / library.metadata_refresh_total) * 100));
  return {
    completed,
    total: library.metadata_refresh_total,
    percent,
    failed: library.metadata_refresh_failed,
  };
}

export function activityProgress(activity: Pick<SystemActivity, 'completed_items' | 'total_items' | 'failed_items'>): {
  completed: number;
  total: number;
  failed: number;
  percent: number;
} {
  const total = Math.max(0, activity.total_items);
  const completed = Math.min(total, Math.max(0, activity.completed_items));
  const failed = Math.max(0, activity.failed_items);
  const percent = total > 0 ? Math.min(100, Math.max(0, (completed / total) * 100)) : 0;
  return { completed, total, failed, percent };
}

export function metadataRefreshActivityProgressForLibrary(libraryId: number): {
  completed: number;
  total: number;
  failed: number;
  percent: number;
} | undefined {
  const activities = activeMetadataRefreshActivities().filter((activity) => activity.library_id === libraryId);
  if (!activities.length) {
    return undefined;
  }

  const totals = activities.reduce((summary, activity) => {
    const progress = activityProgress(activity);
    return {
      completed: summary.completed + progress.completed,
      total: summary.total + progress.total,
      failed: summary.failed + progress.failed,
    };
  }, { completed: 0, total: 0, failed: 0 });
  if (totals.total <= 0) {
    return undefined;
  }

  return {
    ...totals,
    percent: Math.min(100, Math.max(0, (totals.completed / totals.total) * 100)),
  };
}

/** Counts active metadata refresh work items associated with a library. */
export function activeLibraryPendingRefreshCount(libraryId: number): number {
  return activeMetadataRefreshActivities()
    .filter((activity) => activity.library_id === libraryId)
    .reduce((total, activity) => {
      const remaining = Math.max(0, activity.total_items - activity.completed_items - activity.failed_items);
      return total + remaining;
    }, 0);
}

export function libraryHasActiveMetadataRefresh(libraryId: number): boolean {
  return activeMetadataRefreshActivities().some((activity) => activity.library_id === libraryId);
}

export function currentLogFilterRequest(): { level?: string; module?: string; search?: string; since?: string; until?: string; limit: number } {
  return {
    level: state.logFilters.level || undefined,
    module: state.logFilters.module || undefined,
    search: state.logFilters.search || undefined,
    since: state.logFilters.since || undefined,
    until: state.logFilters.until || undefined,
    limit: 200,
  };
}

export function snapshotJson(value: unknown): string {
  return JSON.stringify(value);
}
