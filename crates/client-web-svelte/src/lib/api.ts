// Data layer for the Svelte PoC.
//
// This deliberately mirrors the vanilla client's contract
// (../client-web/src/api.ts) for the logs endpoint: same types, same request
// shape, same VITE_USE_MOCK_API toggle and same mock data. The goal is to show
// the data layer ports nearly verbatim — for a full migration, ../client-web's
// api.ts and mockApi.ts would be copied in wholesale.

export interface LogEntry {
  timestamp: string;
  level: string;
  module: string;
  source_file_path: string;
  line_number?: number;
  message: string;
}

export interface LogEntriesResponse {
  log_path: string;
  entries: LogEntry[];
}

export interface LogFilters {
  level: string;
  module: string;
  search: string;
  since: string;
  until: string;
}

export const EMPTY_LOG_FILTERS: LogFilters = {
  level: '',
  module: '',
  search: '',
  since: '',
  until: '',
};

// Same toggle as the vanilla client. `vite dev --mode mock` loads .env.mock.
const USE_MOCK_API = import.meta.env.VITE_USE_MOCK_API === 'true';

export function isMockApi(): boolean {
  return USE_MOCK_API;
}

function resolveApiBase(): string {
  // Matches the vanilla client's precedence: env override -> origin.
  const fromEnv = (import.meta.env.VITE_API_BASE_URL as string | undefined)?.trim();
  if (fromEnv) {
    return fromEnv;
  }
  return globalThis.location.origin;
}

async function requestJson<T>(method: string, path: string, body?: unknown): Promise<T> {
  const headers: Record<string, string> = {};
  if (body !== undefined) {
    headers['Content-Type'] = 'application/json';
  }
  const token = globalThis.localStorage.getItem('koko-client-web-auth-token');
  if (token) {
    headers.Authorization = `Bearer ${token}`;
  }
  const response = await fetch(resolveApiBase() + path, {
    method,
    headers,
    body: body !== undefined ? JSON.stringify(body) : undefined,
  });
  if (!response.ok) {
    throw new Error(`${method} ${path} failed: ${response.status} ${response.statusText}`);
  }
  return (await response.json()) as T;
}

export async function getLogs(filters?: {
  level?: string;
  module?: string;
  search?: string;
  since?: string;
  until?: string;
  limit?: number;
}): Promise<LogEntriesResponse> {
  if (USE_MOCK_API) {
    return getMockLogs(
      filters?.level,
      filters?.module,
      filters?.search,
      filters?.since,
      filters?.until,
      filters?.limit ?? 200,
    );
  }
  const params = new URLSearchParams();
  if (filters?.level?.trim()) {
    params.set('level', filters.level.trim());
  }
  if (filters?.module?.trim()) {
    params.set('module', filters.module.trim());
  }
  if (filters?.search?.trim()) {
    params.set('search', filters.search.trim());
  }
  if (filters?.since?.trim()) {
    params.set('since', filters.since.trim());
  }
  if (filters?.until?.trim()) {
    params.set('until', filters.until.trim());
  }
  if (typeof filters?.limit === 'number' && Number.isFinite(filters.limit)) {
    params.set('limit', String(filters.limit));
  }
  const suffix = params.toString() ? `?${params.toString()}` : '';
  return requestJson<LogEntriesResponse>('GET', `/api/v1/settings/logs${suffix}`);
}

// --- Mock implementation (copied from ../client-web/src/mockApi.ts getMockLogs) ---

export function getMockLogs(
  level?: string,
  moduleFilter?: string,
  search?: string,
  since?: string,
  until?: string,
  limit = 200,
): LogEntriesResponse {
  const sinceTime = since ? new Date(since).getTime() : Number.NaN;
  const untilTime = until ? new Date(until).getTime() : Number.NaN;
  const entries: LogEntry[] = [
    {
      timestamp: '2026-04-22T09:12:35.853-04:00',
      level: 'INFO',
      module: 'koko::web::routes::media',
      source_file_path: 'src/web/routes/media.rs',
      line_number: 540,
      message:
        'Completed TMDB metadata refresh for media item 201 "Mock Show" (show) in library 2 [Mock Show]',
    },
    {
      timestamp: '2026-04-22T09:12:00.810-04:00',
      level: 'WARN',
      module: 'koko::web::routes::media',
      source_file_path: 'src/web/routes/media.rs',
      line_number: 589,
      message:
        'Failed to fetch refreshed TMDB metadata snapshot for media item 417 "Season 1" (season) in library 2 [The Simpsons/Season 1] using target tv:456:season:1 (tv_season): TMDB season lookup failed with status 404 Not Found',
    },
    {
      timestamp: '2026-04-22T09:10:49.079-04:00',
      level: 'DEBUG',
      module: 'reqwest::connect',
      source_file_path: 'src/connect.rs',
      line_number: 118,
      message: 'starting new connection: https://api.themoviedb.org/',
    },
  ].filter((entry) => {
    const levelMatches = level ? entry.level.toLowerCase() === level.toLowerCase() : true;
    const moduleMatches = moduleFilter
      ? entry.module.toLowerCase().includes(moduleFilter.toLowerCase())
      : true;
    const searchMatches = search
      ? `${entry.message} ${entry.module} ${entry.source_file_path}`
          .toLowerCase()
          .includes(search.toLowerCase())
      : true;
    const timestamp = new Date(entry.timestamp).getTime();
    const sinceMatches = Number.isNaN(sinceTime) || timestamp >= sinceTime;
    const untilMatches = Number.isNaN(untilTime) || timestamp <= untilTime;
    return levelMatches && moduleMatches && searchMatches && sinceMatches && untilMatches;
  });

  return {
    log_path: 'C:/Users/Mock/AppData/Local/Koko/data/koko.log',
    entries: entries.slice(0, Math.max(1, limit)),
  };
}
