// Activities store — system activities + log filters/response.
import {
  getSystemActivities,
  getLogs,
  type SystemActivitiesResponse,
  type LogEntriesResponse,
  type LogFilters,
  EMPTY_LOG_FILTERS,
} from '$lib/api';

class ActivitiesStore {
  systemActivities = $state<SystemActivitiesResponse | undefined>(undefined);
  logsResponse = $state<LogEntriesResponse | undefined>(undefined);
  logFilters = $state<LogFilters>({ ...EMPTY_LOG_FILTERS });
  loading = $state(false);

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
}

export const activities = new ActivitiesStore();
