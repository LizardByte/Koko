// Pure formatting helpers ported from ../client-web/src/app/format.ts.
// In Svelte, escapeHtml is unnecessary (auto-escaped), so only the duration/
// timestamp/file-size helpers carry over.
export function formatDuration(ms?: number): string {
  if (!ms || ms <= 0) return '--';
  const totalSeconds = Math.floor(ms / 1000);
  const hours = Math.floor(totalSeconds / 3600);
  const minutes = Math.floor((totalSeconds % 3600) / 60);
  const seconds = totalSeconds % 60;
  if (hours > 0) {
    return `${hours}h ${minutes}m`;
  }
  if (minutes > 0) {
    return `${minutes}m`;
  }
  return `${seconds}s`;
}

export function formatTimestamp(iso: string): string {
  const date = new Date(iso);
  if (Number.isNaN(date.getTime())) return iso;
  return date.toLocaleString();
}

export function formatFileSize(bytes?: number): string {
  if (!bytes || bytes <= 0) return '--';
  const units = ['B', 'KB', 'MB', 'GB', 'TB'];
  let value = bytes;
  let unitIndex = 0;
  while (value >= 1024 && unitIndex < units.length - 1) {
    value /= 1024;
    unitIndex++;
  }
  return `${value.toFixed(value >= 10 || unitIndex === 0 ? 0 : 1)} ${units[unitIndex]}`;
}

export function formatRating(rating?: number): string {
  if (typeof rating !== 'number' || !Number.isFinite(rating)) return '--';
  return rating.toFixed(1);
}
