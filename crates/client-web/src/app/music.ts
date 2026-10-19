import type { MediaItemDetail, MediaItemSummary } from '../api';
import { escapeHtml, formatDuration } from './format';
import { renderButtonContent } from './ui';

/** Order discs and tracks numerically, keeping unnumbered tracks at the end. */
export function orderedAlbumTracks(items: MediaItemSummary[]): MediaItemSummary[] {
  return items.filter((item) => item.item_type === 'track').sort((left, right) =>
    (left.music?.disc_number ?? 1) - (right.music?.disc_number ?? 1)
    || (left.music?.track_number ?? Number.MAX_SAFE_INTEGER) - (right.music?.track_number ?? Number.MAX_SAFE_INTEGER)
    || left.display_title.localeCompare(right.display_title, undefined, { numeric: true })
    || left.id - right.id);
}

/** Only available tracks belong in an album playback queue. */
export function albumPlaybackQueue(album: MediaItemDetail, startTrackId?: number): number[] {
  const tracks = orderedAlbumTracks(album.children).filter((track) => track.playable && !track.missing_since);
  const start = startTrackId === undefined ? 0 : tracks.findIndex((track) => track.id === startTrackId);
  return start < 0 ? [] : tracks.slice(start).map((track) => track.id);
}

/** Albums use a compact track list with individual playback and detail actions. */
export function renderAlbumTracks(album: MediaItemDetail): string {
  const tracks = orderedAlbumTracks(album.children);
  if (!tracks.length) return '';
  const multipleDiscs = new Set(tracks.map((track) => track.music?.disc_number ?? 1)).size > 1;
  return `<section class="panel page-panel item-section" data-nav-section="tracks" data-nav-label="Tracks">
    <div class="section-heading"><h3>Tracks</h3><span class="muted">${tracks.length} tracks</span></div>
    <div class="music-track-list">${tracks.map((track) => {
      const number = [multipleDiscs ? track.music?.disc_number ?? 1 : undefined, track.music?.track_number ?? '–'].filter((value) => value !== undefined).join('.');
      return `<div class="music-track-row ${track.missing_since ? 'is-missing' : ''}">
        <span class="music-track-number">${escapeHtml(number)}</span>
        <button type="button" class="music-track-title" data-item-id="${track.id}"><strong>${escapeHtml(track.display_title)}</strong><small>${escapeHtml(track.music?.artist ?? '')}</small></button>
        <span class="muted">${escapeHtml(formatDuration(track.duration_ms))}</span>
        <button type="button" class="secondary-button icon-only" data-play-music-track-id="${track.id}" aria-label="Play ${escapeHtml(track.display_title)}" ${!track.playable || track.missing_since ? 'disabled' : ''}>${renderButtonContent('', 'play')}</button>
      </div>`;
    }).join('')}</div>
  </section>`;
}
