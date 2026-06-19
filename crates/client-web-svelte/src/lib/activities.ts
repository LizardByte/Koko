// Mirrors currentLogFilterRequest() from ../client-web/src/app/activities.ts.
// Kept as a standalone helper to demonstrate that the derived-state functions
// port over as plain functions (no framework coupling).
import type { LogFilters } from './api';

export function buildLogFilterRequest(filters: LogFilters): {
  level?: string;
  module?: string;
  search?: string;
  since?: string;
  until?: string;
  limit: number;
} {
  return {
    level: filters.level || undefined,
    module: filters.module || undefined,
    search: filters.search || undefined,
    since: filters.since || undefined,
    until: filters.until || undefined,
    limit: 200,
  };
}
