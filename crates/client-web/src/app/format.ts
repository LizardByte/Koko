/** Formats a Unix timestamp in seconds for display. */
export function formatTimestamp(timestamp?: number): string {
  if (!timestamp) {
    return 'Unknown';
  }

  return new Date(timestamp * 1000).toLocaleString('en-US');
}

/** Formats a millisecond duration as H:MM:SS or M:SS. */
export function formatDuration(durationMs?: number): string {
  if (!durationMs) {
    return 'Unknown';
  }

  const totalSeconds = Math.floor(durationMs / 1000);
  const hours = Math.floor(totalSeconds / 3600);
  const minutes = Math.floor((totalSeconds % 3600) / 60);
  const seconds = totalSeconds % 60;

  if (hours > 0) {
    return `${hours}:${String(minutes).padStart(2, '0')}:${String(seconds).padStart(2, '0')}`;
  }

  return `${minutes}:${String(seconds).padStart(2, '0')}`;
}

/** Formats a media playback position in seconds. */
export function formatMediaTime(seconds: number): string {
  if (!Number.isFinite(seconds) || seconds < 0) {
    return '0:00';
  }

  return formatDuration(Math.floor(seconds * 1000));
}

/** Formats a byte count using binary units. */
export function formatFileSize(fileSize?: number): string {
  if (!fileSize) {
    return 'Unknown';
  }

  const units = ['B', 'KB', 'MB', 'GB', 'TB'];
  let size = fileSize;
  let unitIndex = 0;
  while (size >= 1024 && unitIndex < units.length - 1) {
    size /= 1024;
    unitIndex += 1;
  }

  return `${size.toFixed(size >= 10 || unitIndex === 0 ? 0 : 1)} ${units[unitIndex]}`;
}

/** Formats a bitrate using bps, kbps, or Mbps. */
export function formatBitRate(bitRate?: number): string {
  if (!bitRate) {
    return 'Unknown';
  }

  if (bitRate >= 1_000_000) {
    return `${(bitRate / 1_000_000).toFixed(bitRate >= 10_000_000 ? 0 : 1)} Mbps`;
  }

  if (bitRate >= 1_000) {
    return `${Math.round(bitRate / 1_000)} kbps`;
  }

  return `${bitRate} bps`;
}

/** Escapes text that is interpolated into hand-built HTML strings. */
export function escapeHtml(value: string): string {
  return value
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;')
    .replace(/'/g, '&#39;');
}
