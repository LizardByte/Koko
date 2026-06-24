// Pure formatting helpers — full-fidelity port of ../client-web/src/app/format.ts.
// (escapeHtml is unnecessary in Svelte: interpolations are auto-escaped.)

export function formatTimestamp(unixSeconds?: number): string {
  if (typeof unixSeconds !== 'number' || !Number.isFinite(unixSeconds)) {
    return 'Unknown';
  }
  return new Date(unixSeconds * 1000).toLocaleString('en-US');
}

export function formatDuration(ms?: number): string {
  if (typeof ms !== 'number' || !Number.isFinite(ms) || ms <= 0) {
    return '--';
  }
  const totalSeconds = Math.floor(ms / 1000);
  const hours = Math.floor(totalSeconds / 3600);
  const minutes = Math.floor((totalSeconds % 3600) / 60);
  const seconds = totalSeconds % 60;
  if (hours > 0) {
    return `${hours}:${String(minutes).padStart(2, '0')}:${String(seconds).padStart(2, '0')}`;
  }
  return `${minutes}:${String(seconds).padStart(2, '0')}`;
}

/** Formats a position in seconds as a media timestamp (e.g. "1:23:45"). */
export function formatMediaTime(seconds: number): string {
  if (!Number.isFinite(seconds) || seconds < 0) {
    return '0:00';
  }
  return formatDuration(Math.floor(seconds * 1000));
}

export function formatBitRate(bitsPerSecond?: number): string {
  if (typeof bitsPerSecond !== 'number' || !Number.isFinite(bitsPerSecond) || bitsPerSecond <= 0) {
    return 'Unknown';
  }
  if (bitsPerSecond >= 1_000_000) {
    return `${(bitsPerSecond / 1_000_000).toFixed(1)} Mbps`;
  }
  if (bitsPerSecond >= 1_000) {
    return `${(bitsPerSecond / 1_000).toFixed(0)} kbps`;
  }
  return `${bitsPerSecond} bps`;
}

export function formatFileSize(bytes?: number): string {
  if (typeof bytes !== 'number' || !Number.isFinite(bytes) || bytes <= 0) {
    return 'Unknown';
  }
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
  if (typeof rating !== 'number' || !Number.isFinite(rating)) {
    return '--';
  }
  return rating.toFixed(1);
}

/** Formats a 'YYYY-MM-DD' as e.g. "Sep 2, 1964" (en-US). */
export function formatPersonDate(isoDate?: string): string {
  if (!isoDate) {
    return '';
  }
  const date = new Date(isoDate);
  if (Number.isNaN(date.getTime())) {
    return isoDate;
  }
  return date.toLocaleDateString('en-US', { year: 'numeric', month: 'short', day: 'numeric' });
}

/** "<n> years old" or "<n> at death" — used by the person hero. */
export function personAgeLabel(birthday?: string, deathday?: string): string | undefined {
  if (!birthday) {
    return undefined;
  }
  const birth = new Date(birthday);
  if (Number.isNaN(birth.getTime())) {
    return undefined;
  }
  const end = deathday ? new Date(deathday) : new Date();
  const years = Math.floor((end.getTime() - birth.getTime()) / (365.25 * 24 * 3600 * 1000));
  return deathday ? `${years} at death` : `${years} years old`;
}
