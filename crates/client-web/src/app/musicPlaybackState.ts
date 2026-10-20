/** Shares live audio playback and progress overlays with item and playlist views. */

import type { MediaItemSummary, MusicPlaylistDetail } from '../api';

export interface MusicPlaybackState {
  userId?: number;
  queueId?: number;
  queueIndex?: number;
  queueItems?: MediaItemSummary[];
  item: MediaItemSummary;
  positionMs: number;
  durationMs: number;
  paused: boolean;
  ended: boolean;
}

let active: MusicPlaybackState | undefined;
const progress = new Map<number, Partial<MediaItemSummary>>();

/** Overlay local playback until the next server response catches up. */
export function musicPlaybackItem<T extends MediaItemSummary>(item: T): T {
  return { ...item, ...progress.get(item.id) };
}

export function activeMusicPlayback(): MusicPlaybackState | undefined { return active; }

/** Keep a queue's detail page in the actual playing order, including repeated songs. */
export function musicPlaybackPlaylist(detail: MusicPlaylistDetail): MusicPlaylistDetail {
  if (active?.queueId !== detail.playlist.id) return detail;
  const items = active.queueItems ?? detail.items;
  const index = active.queueIndex ?? detail.playlist.current_index ?? 0;
  return { ...detail, items, playlist: { ...detail.playlist, current_index: index, position_ms: active.positionMs, completed: active.ended && index === items.length - 1 } };
}

export interface MusicCardTarget { itemId?: number; albumId?: number; queueId?: number }
export type MusicCardStatus = 'playing' | 'paused';

/** Match queue identity separately from item identity, including paused playback. */
export function musicCardStatus(target: MusicCardTarget): MusicCardStatus | undefined {
  if (!active || active.ended) return undefined;
  const matches = target.queueId !== undefined
    ? active.queueId === target.queueId
    : target.itemId === active.item.id
      || (active.queueId === undefined && target.albumId !== undefined && target.albumId === active.item.parent_id);
  if (!matches) return undefined;
  return active.paused ? 'paused' : 'playing';
}

export function setMusicPlayback(playback: MusicPlaybackState | undefined): void {
  active = playback;
  if (!playback) return;
  const { item, positionMs, durationMs, ended } = playback;
  progress.set(item.id, {
    playback_position_ms: ended ? durationMs : positionMs,
    playback_duration_ms: durationMs,
    playback_completed: ended,
    watch_count: ended ? (item.watch_count ?? 0) + 1 : item.watch_count,
    last_played_at: Date.now() / 1000,
  });
}

/** Playback from another account must never appear in this account's pages. */
export function clearMusicPlayback(): void { active = undefined; progress.clear(); }
