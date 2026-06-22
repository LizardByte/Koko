import type { MediaItemExtra } from '../api';
import { formatMediaTime } from './format';
import type { TrailerOption } from './types';

/** Converts a media-extra type identifier into a display label. */
export function mediaExtraTypeLabel(extraType: string): string {
  return extraType
    .split('_')
    .filter(Boolean)
    .map((part) => part.slice(0, 1).toUpperCase() + part.slice(1))
    .join(' ') || 'Extra';
}

/** Returns the best title for an extra, falling back to its type label. */
export function mediaExtraTitle(extra: MediaItemExtra): string {
  return extra.title?.trim() || mediaExtraTypeLabel(extra.extra_type);
}

/** Formats an extra duration for display. */
export function mediaExtraDurationLabel(extra: MediaItemExtra): string {
  return typeof extra.duration_seconds === 'number' && extra.duration_seconds > 0
    ? formatMediaTime(extra.duration_seconds)
    : 'Unknown length';
}

/** Converts a trailer extra into the overlay player's trailer option shape. */
export function mediaExtraToTrailerOption(extra: MediaItemExtra): TrailerOption {
  return {
    title: mediaExtraTitle(extra),
    url: extra.url,
    label: mediaExtraTypeLabel(extra.extra_type),
  };
}
