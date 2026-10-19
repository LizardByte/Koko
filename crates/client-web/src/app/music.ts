import { getArtworkUrl, type MediaItemDetail, type MediaItemSummary, type MusicPlaylistDetail } from '../api';
import { escapeHtml, formatDuration } from './format';
import { renderButtonContent, renderIcon } from './ui';
import { playbackProgressPercent } from './playbackProgress';
import { activeMusicPlayback, musicPlaybackItem, musicPlaybackPlaylist } from './musicPlaybackState';

export function renderTrackListeningStatus(track: MediaItemSummary): string {
  const progress = playbackProgressPercent(track);
  if (track.playback_completed || (track.watch_count ?? 0) > 0 && progress === undefined) {
    return `<span class="track-listening-status is-complete" title="Listened" aria-label="Listened">${renderIcon('check')}</span>`;
  }
  return progress !== undefined
    ? `<span class="track-listening-status" style="--watch-progress:${progress}%" title="In progress: ${progress}% listened" aria-label="In progress: ${progress}% listened"></span>`
    : '';
}

export function renderActiveAlbumButton(albumId: number): string {
  const playback = activeMusicPlayback();
  if (!playback || playback.ended || playback.item.parent_id !== albumId) return '';
  return `<button type="button" data-toggle-music-album="${albumId}">${renderButtonContent(playback.paused ? 'Resume album' : 'Pause album', playback.paused ? 'play' : 'pause', 'start', playback.item.display_title)}</button>`;
}

/** A playlist transport controls the existing queue rather than restarting its first song. */
export function playlistPlaybackButtonContent(detail: MusicPlaylistDetail): string {
  const playback = activeMusicPlayback();
  const kind = detail.playlist.temporary ? 'radio' : 'playlist';
  if (playback?.queueId === detail.playlist.id && !playback.ended) {
    return renderButtonContent(`${playback.paused ? 'Resume' : 'Pause'} ${kind}`, playback.paused ? 'play' : 'pause', 'start', playback.item.display_title);
  }
  const continuing = !detail.playlist.completed && ((detail.playlist.current_index ?? 0) > 0 || (detail.playlist.position_ms ?? 0) > 0);
  const current = continuing ? detail.items[detail.playlist.current_index ?? 0] : undefined;
  return renderButtonContent(`${continuing ? 'Continue' : 'Play'} ${kind}`, 'play', 'start', current?.display_title);
}

/** Playlist positions preserve the mix order and identify repeated occurrences separately. */
export function renderPlaylistTracks(saved: MusicPlaylistDetail): string {
  const detail = musicPlaybackPlaylist(saved);
  const playback = activeMusicPlayback();
  return `<div class="music-track-list">${detail.items.map((track, index) => {
    const isCurrent = playback?.queueId === detail.playlist.id && !playback.ended && (playback.queueIndex ?? detail.playlist.current_index ?? 0) === index;
    return renderMusicTrackRow(track, {
      number: String(index + 1), menuId: `playlist-${detail.playlist.id}-${index}`, isCurrent, cover: true,
      rowAttributes: `data-music-playlist-id="${detail.playlist.id}" data-music-playlist-index="${index}"`,
      playAttributes: `data-play-playlist-track="${detail.playlist.id}" data-playlist-index="${index}"`,
    });
  }).join('')}</div>`;
}

function renderMusicTrackRow(savedTrack: MediaItemSummary, options: { number: string; menuId: string; isCurrent: boolean; cover?: boolean; rowAttributes?: string; playAttributes: string }): string {
  const track = musicPlaybackItem(savedTrack);
  const paused = activeMusicPlayback()?.paused ?? true;
  const artwork = getArtworkUrl(track.artwork_item_id ?? track.id, 'poster', track.artwork_updated_at);
  const subtitle = [track.music?.artist, options.cover ? track.music?.album : undefined].filter(Boolean).join(' · ');
  return `<div class="music-track-row ${options.cover ? 'music-playlist-track' : ''} ${track.missing_since ? 'is-missing' : ''} ${options.isCurrent ? 'is-current' : ''}" data-music-track-id="${track.id}" ${options.rowAttributes ?? ''} ${options.isCurrent ? 'aria-current="true"' : ''}>
    <span class="music-track-number">${escapeHtml(options.number)}</span>
    <span class="music-track-status">${renderTrackListeningStatus(track)}</span>
    ${options.cover ? `<span class="music-track-cover" aria-hidden="true" style="background-image:url('${escapeHtml(artwork)}');"></span>` : ''}
    <button type="button" class="music-track-title" data-item-id="${track.id}"><strong>${escapeHtml(track.display_title)}</strong><small>${escapeHtml(subtitle)}</small></button>
    <span class="muted music-track-duration">${escapeHtml(formatDuration(track.duration_ms))}</span>
    <button type="button" class="secondary-button icon-only" ${options.playAttributes} aria-label="${options.isCurrent && !paused ? 'Pause' : 'Play'} ${escapeHtml(track.display_title)}" ${!track.playable || track.missing_since ? 'disabled' : ''}>${renderButtonContent('', options.isCurrent && !paused ? 'pause' : 'play')}</button>
    <div class="music-song-menu"><button type="button" class="secondary-button" popovertarget="song-actions-${options.menuId}" aria-label="Actions for ${escapeHtml(track.display_title)}">${renderButtonContent('', 'ellipsis')}</button><div popover id="song-actions-${options.menuId}">
      <button type="button" data-music-enqueue="${track.id}" data-music-next="true">Play next</button>
      <button type="button" data-music-enqueue="${track.id}">Add to queue</button>
      <button type="button" data-music-radio="${track.id}">Start radio</button>
    </div></div>
  </div>`;
}

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
    <div class="music-track-list">${tracks.map((savedTrack) => {
      const track = savedTrack;
      const playback = activeMusicPlayback();
      const isCurrent = playback?.item.id === track.id && !playback.ended;
      const number = [multipleDiscs ? track.music?.disc_number ?? 1 : undefined, track.music?.track_number ?? '–'].filter((value) => value !== undefined).join('.');
      return renderMusicTrackRow(track, { number, menuId: `${album.id}-${track.id}`, isCurrent, playAttributes: `data-play-music-track-id="${track.id}"` });
    }).join('')}</div>
  </section>`;
}
