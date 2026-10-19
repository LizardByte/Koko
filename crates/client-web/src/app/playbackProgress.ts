import type { MediaItemDetail, MediaItemSummary } from '../api';

/** Returns a saved playback position only when it is useful as a resume target. */
export function resumablePlaybackPositionMs(item: MediaItemDetail): number {
  if (item.playback_completed) {
    return 0;
  }
  const positionMs = item.playback_position_ms ?? 0;
  const durationMs = item.playback_duration_ms ?? item.duration_ms ?? 0;
  const minimum=item.media_kind==='audio' ? 1000 : 30000;
  if (positionMs < minimum) {
    return 0;
  }
  if (durationMs > 0 && durationMs - positionMs < (item.media_kind==='audio' ? 0 : 30000)) {
    return 0;
  }

  return positionMs;
}

/** Returns a 1-99 watch-progress percentage for cards, or undefined when hidden. */
export function playbackProgressPercent(item: MediaItemSummary): number | undefined {
  if (item.playback_completed) {
    return undefined;
  }
  const positionMs = item.playback_position_ms ?? 0;
  const durationMs = item.playback_duration_ms ?? item.duration_ms ?? 0;
  if (positionMs < (item.media_kind==='audio' ? 1000 : 30000) || durationMs <= 0 || durationMs - positionMs < (item.media_kind==='audio' ? 0 : 30000)) {
    return undefined;
  }

  return Math.min(99, Math.max(1, Math.round((positionMs / durationMs) * 100)));
}

export function playbackAction(item:MediaItemSummary):'listened'|'read'|'watched' {
  if (item.media_kind === 'audio' || ['artist', 'album', 'track'].includes(item.item_type)) return 'listened';
  if (item.media_kind === 'book' || item.item_type === 'book') return 'read';
  return 'watched';
}
