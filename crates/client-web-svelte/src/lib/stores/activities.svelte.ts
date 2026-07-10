// Activities store — system activities + log filters/response.
import {
  getSystemActivities,
  getLogs,
  getLibraries,
  getItems,
  type SystemActivitiesResponse,
  type LogEntriesResponse,
  type LogFilters,
  type MediaItemSummary,
  EMPTY_LOG_FILTERS,
} from '$lib/api';
import { libraries } from './libraries.svelte';

class ActivitiesStore {
  systemActivities = $state<SystemActivitiesResponse | undefined>(undefined);
  logsResponse = $state<LogEntriesResponse | undefined>(undefined);
  logFilters = $state<LogFilters>({ ...EMPTY_LOG_FILTERS });
  loading = $state(false);

  // Dashboard state (Phase 5 Step 6). dashboardItems = all items via getItems();
  // metadataDashboardFilters = the filter form state.
  dashboardItems = $state<MediaItemSummary[]>([]);
  dashboardLoading = $state(false);
  metadataDashboardFilters = $state<{ libraryId: string; itemType: string; refreshState: string; search: string }>({
    libraryId: '',
    itemType: '',
    refreshState: '',
    search: '',
  });

  async loadActivities() {
    this.systemActivities = await getSystemActivities();
  }

  async loadLogs() {
    this.loading = true;
    try {
      this.logsResponse = await getLogs({
        level: this.logFilters.level || undefined,
        module: this.logFilters.module || undefined,
        search: this.logFilters.search || undefined,
        since: this.logFilters.since || undefined,
        until: this.logFilters.until || undefined,
        limit: 200,
      });
    } finally {
      this.loading = false;
    }
  }

  setLogFilters(filters: LogFilters) {
    this.logFilters = { ...filters, level: filters.level.toUpperCase() };
  }

  clearLogFilters() {
    this.logFilters = { ...EMPTY_LOG_FILTERS };
  }

  // --- Dashboard (Phase 5 Step 6) ---

  async loadDashboard() {
    this.dashboardLoading = true;
    try {
      this.dashboardItems = await getItems();
    } finally {
      this.dashboardLoading = false;
    }
  }

  setDashboardFilters(filters: { libraryId: string; itemType: string; refreshState: string; search: string }) {
    this.metadataDashboardFilters = { ...filters };
  }

  clearDashboardFilters() {
    this.metadataDashboardFilters = { libraryId: '', itemType: '', refreshState: '', search: '' };
  }

  // --- Auto-refresh polling (Phase 6.5d) ---
  // Mirrors vanilla app.ts:212-255, 724-840. When metadata refresh or library
  // scan activities are in-progress (or items/libraries are pending), poll
  // every 1500ms to update the UI as they complete. Svelte's fine-grained
  // reactivity replaces vanilla's snapshot-diff + maybeRenderAfterAutoRefresh.

  /**
   * Whether any background work warrants polling. Reactive — the layout's $effect
   * reads this to arm/tear-down the timer. Mirrors shouldAutoRefreshMetadata
   * (app.ts:212-243) but simplified: we poll when there are active activities OR
   * pending metadata-refresh counts on libraries.
   */
  get shouldPoll(): boolean {
    const acts = this.systemActivities?.activities ?? [];
    const hasActive = acts.some(
      (a) =>
        (a.category === 'metadata_refresh' || a.category === 'library_scan') &&
        a.state !== 'completed' &&
        a.state !== 'failed',
    );
    const librariesPending = libraries.libraries.some((lib) => (lib.metadata_refresh_pending ?? 0) > 0);
    return hasActive || librariesPending;
  }

  /**
   * Whether any library has never been scanned (status === 'never_scanned').
   * Triggers a separate 1800ms polling loop that re-fetches libraries until
   * the initial scan starts. Mirrors shouldAutoRefreshLibraries (app.ts:172-175).
   */
  get shouldPollLibraries(): boolean {
    return libraries.libraries.some((lib) => lib.status === 'never_scanned');
  }

  /**
   * One poll tick: re-fetch activities + libraries. Page-specific data
   * (item/home) is re-fetched by the stores that own it when their inputs
   * change reactively, so we only need the global bundle here. Called by the
   * layout's self-rescheduling timer.
   */
  async poll() {
    try {
      const [activitiesData, libs] = await Promise.all([
        getSystemActivities(),
        getLibraries(),
      ]);
      this.systemActivities = activitiesData;
      libraries.libraries = libs;
    } catch {
      // Swallow — polling errors shouldn't clobber the UI (vanilla surfaces
      // them via state.error, but a transient poll failure isn't actionable).
    }
  }
}

export const activities = new ActivitiesStore();
