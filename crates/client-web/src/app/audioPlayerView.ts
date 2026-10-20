/** Renders audio controls and queue/lyrics panels and parses timed lyrics. */

import { getArtworkUrl, type MediaItemSummary, type MusicLyrics } from '../api';
import { escapeHtml } from './format';
import { renderIcon } from './ui';
import type { AppIconName } from './types';
import type { AudioQueue, QueueEntry } from './audioQueue';

export interface AudioPlayerViewState {
  queue: AudioQueue;
  item?: MediaItemSummary;
  loading: boolean;
  paused: boolean;
  error?: string;
  volume: number;
  muted: boolean;
  panel?: 'queue' | 'lyrics';
  lyrics?: MusicLyrics;
  lyricsLoading: boolean;
  lyricsError?: string;
  notice?: string;
  saving: boolean;
  lyricsEnabled?: boolean;
  queueName?: string;
}
export function formatAudioTime(seconds: number): string {
  const value = Number.isFinite(seconds) ? Math.max(0, Math.floor(seconds)) : 0;
  const minutes = Math.floor(value / 60),
    remainder = String(value % 60).padStart(2, '0');
  return `${minutes}:${remainder}`;
}
function button(
  action: string,
  label: string,
  icon: AppIconName,
  active = false,
  disabled = false,
): string {
  const pressed = ['shuffle', 'repeat', 'queue', 'lyrics'].includes(action)
    ? `aria-pressed="${active}"`
    : '';
  const expanded = ['queue', 'lyrics'].includes(action)
    ? `aria-controls="music-panel" aria-expanded="${active}"`
    : '';
  return `<button type="button" class="player-icon-button music-control ${active ? 'is-active' : ''}" data-music-action="${action}" aria-label="${escapeHtml(label)}" title="${escapeHtml(label)}" ${pressed} ${expanded} ${disabled ? 'disabled' : ''}>${renderIcon(icon)}</button>`;
}
function playIcon(view: AudioPlayerViewState): AppIconName {
  if (view.loading) return 'refresh-cw';
  return view.paused ? 'play' : 'pause';
}
function feedback(view: AudioPlayerViewState): string {
  if (!view.error) return '';
  return `<div class="music-feedback" role="alert">${escapeHtml(view.error)} ${button('retry', 'Retry playback', 'refresh-cw')}</div>`;
}
export function renderAudioPlayer(view: AudioPlayerViewState): string {
  const { queue, item } = view;
  if (!item) return '';
  const artist = item.music?.artist ?? item.display_subtitle ?? '';
  const repeatLabel = {
    off: 'Repeat off',
    all: 'Repeat all',
    one: 'Repeat one',
  }[queue.repeat];
  const panel = view.panel ? renderPanel(view) : '';
  const inlineLyrics = view.lyricsEnabled
    ? '<div class="music-inline-lyrics" hidden><button type="button" data-music-action="lyrics" data-current-lyric aria-label="Expand current lyrics"></button></div>'
    : '';
  return `${panel}<section id="music-dock" class="music-dock" aria-label="Audio player" data-nav-section="music-player">
    <div class="music-now-playing"><button class="music-cover" type="button" data-music-open-item="${item.parent_id ?? item.id}" aria-label="Open album">${renderIcon('music')}<img src="${escapeHtml(getArtworkUrl(item.artwork_item_id ?? item.parent_id ?? item.id, 'poster', item.artwork_updated_at))}" alt="" /></button>
      <div><button class="music-title" type="button" data-music-open-item="${item.id}">${escapeHtml(item.display_title)}</button><span class="muted">${escapeHtml(artist)}</span></div></div>
    <div class="music-transport"><div class="music-buttons">
      ${button('shuffle', 'Shuffle', 'shuffle', queue.shuffle)}${button('previous', 'Previous song', 'skip-back')}${button('back', 'Back 10 seconds', 'rotate-ccw')}
      ${button('play', view.paused ? 'Play audio' : 'Pause audio', playIcon(view), false, view.loading)}
      ${button('forward', 'Forward 10 seconds', 'rotate-ccw')}${button('next', 'Next song', 'skip-forward', false, queue.cursor >= queue.order.length - 1 && queue.repeat !== 'all')}
      ${button('repeat', repeatLabel, queue.repeat === 'one' ? 'repeat-1' : 'repeat', queue.repeat !== 'off')}
      </div><div class="music-timeline"><span data-music-elapsed>0:00</span><input class="player-progress" data-music-seek type="range" min="0" max="1000" step="1" value="0" aria-label="Audio position"/><span data-music-duration>${formatAudioTime((item.duration_ms ?? 0) / 1000)}</span></div></div>
    <div class="music-tools">${button('lyrics', 'Lyrics', 'mic-2', view.panel === 'lyrics')}${button('queue', 'Playing queue', 'list-music', view.panel === 'queue')}
      ${button('mute', view.muted ? 'Unmute audio' : 'Mute audio', view.muted ? 'volume-x' : 'volume-2')}
      <input class="player-volume" data-music-volume type="range" min="0" max="1" step="0.01" value="${view.muted ? 0 : view.volume}" aria-label="Audio volume"/>
      ${button('stop', 'Stop audio', 'x')}</div>
    ${inlineLyrics}${feedback(view)}
  </section>`;
}
function renderQueueEntry(
  view: AudioPlayerViewState,
  entry: QueueEntry,
  index: number,
): string {
  const current = entry.key === view.queue.current?.key;
  const move =
    index > view.queue.cursor
      ? `<button type="button" class="music-control" data-music-move="${entry.key}:-1" aria-label="Move song up">${renderIcon('chevron-up')}</button><button type="button" class="music-control" data-music-move="${entry.key}:1" aria-label="Move song down">${renderIcon('chevron-down')}</button>`
      : '';
  const remove = current
    ? ''
    : `<button type="button" class="music-control" data-music-remove="${entry.key}" aria-label="Remove ${escapeHtml(entry.item.display_title)}">${renderIcon('x')}</button>`;
  return `<div class="music-queue-entry ${current ? 'is-current' : ''}">
    <button type="button" data-music-queue-key="${entry.key}" ${current ? 'aria-current="true"' : ''}><span>${index + 1}</span><span><strong>${escapeHtml(entry.item.display_title)}</strong><small>${escapeHtml(entry.item.music?.artist ?? '')}</small></span></button>${move}${remove}</div>`;
}
function renderQueue(view: AudioPlayerViewState): string {
  const entries = view.queue.ordered
    .map((entry, index) => renderQueueEntry(view, entry, index))
    .join('');
  return `<div class="music-queue-list">${entries}</div>
    <footer><button type="button" class="secondary-button" data-music-action="clear">Clear upcoming</button><form data-music-save><label>Playlist name<input id="music-playlist-name" name="name" required maxlength="200" autocomplete="off" placeholder="My playlist"/></label><button type="submit" ${view.saving ? 'disabled' : ''}>${view.saving ? 'Saving…' : 'Save as playlist'}</button></form></footer>`;
}
function renderPanel(view: AudioPlayerViewState): string {
  const title = view.panel === 'queue' ? 'Playing queue' : 'Lyrics';
  const queueName = view.queueName ?? `${view.queue.order.length} songs`;
  const subtitle =
    view.panel === 'queue' ? queueName : (view.item?.display_title ?? '');
  const contents =
    view.panel === 'lyrics'
      ? `<div class="music-lyrics-scroll">${renderLyrics(view)}</div><footer>${button('follow-lyrics', 'Jump to current lyrics', 'locate-fixed')}</footer>`
      : renderQueue(view);
  const notice =
    view.notice && view.panel === 'queue'
      ? `<p class="music-feedback" role="status">${escapeHtml(view.notice)}</p>`
      : '';
  return `<section id="music-panel" class="music-panel" data-nav-section="music-panel" aria-label="${title}"><header><div><h3 id="music-panel-title" tabindex="-1">${title}</h3><p class="muted">${escapeHtml(subtitle)}</p></div>${button('close-panel', 'Close audio panel', 'x')}</header>${contents}${notice}</section>`;
}
function lyricContents(lyrics: MusicLyrics): string {
  if (lyrics.instrumental) return '<p>Instrumental</p>';
  const lines = parseTimedLyrics(lyrics.synced_lyrics ?? '');
  if (lines.length) {
    const markup = lines
      .map(
        (line) =>
          `<p data-lyric-time="${line.time}">${escapeHtml(line.text)}</p>`,
      )
      .join('');
    return `<div class="music-lyrics">${markup}</div>`;
  }
  return `<pre class="music-lyrics">${escapeHtml(lyrics.plain_lyrics ?? 'No lyrics are available for this song.')}</pre>`;
}
function renderLyrics(view: AudioPlayerViewState): string {
  if (view.lyricsLoading) return '<p role="status">Loading saved lyrics…</p>';
  if (view.lyricsError)
    return `<p role="status">${escapeHtml(view.lyricsError)}</p>`;
  if (!view.lyrics) return '<p>No saved lyrics have been loaded.</p>';
  const lyrics = view.lyrics;
  if (
    !lyrics.instrumental &&
    !lyrics.synced_lyrics?.trim() &&
    !lyrics.plain_lyrics?.trim()
  )
    return '<p>No saved lyrics are available. Match the album and track, select LRCLIB in the library’s metadata sources, and refresh this track’s metadata to look for lyrics.</p>';
  return `${lyricContents(lyrics)}<p class="muted">Lyrics from <a href="${escapeHtml(lyrics.source_url)}" target="_blank" rel="noreferrer">${escapeHtml(lyrics.source)}</a></p>`;
}
/** Empty timestamped lines clear the inline lyric during instrumental gaps. */
export function currentLyricText(text: string, seconds: number): string {
  const lines = parseTimedLyrics(text);
  for (let index = lines.length - 1; index >= 0; index--) {
    if (lines[index].time <= seconds) return lines[index].text;
  }
  return '';
}
function stripLrcTags(line: string): string {
  let result = '',
    start = 0;
  while (start < line.length) {
    const open = line.indexOf('[', start);
    if (open < 0) return (result + line.slice(start)).trim();
    const close = line.indexOf(']', open + 1);
    if (close < 0) return (result + line.slice(start)).trim();
    result += line.slice(start, open);
    start = close + 1;
  }
  return result.trim();
}
/** Support multiple timestamps per line, decimal seconds, and LRC offset metadata. */
export function parseTimedLyrics(
  text: string,
): Array<{ time: number; text: string }> {
  const offset = Number(/\[offset:([+-]?\d+)\]/i.exec(text)?.[1] ?? 0) / 1000;
  return text
    .split(/\r?\n/)
    .flatMap((line) => {
      const matches = [...line.matchAll(/\[(\d+):(\d+(?:\.\d+)?)\]/g)];
      const content = stripLrcTags(line);
      return matches.map((match) => ({
        time: Math.max(0, Number(match[1]) * 60 + Number(match[2]) + offset),
        text: content,
      }));
    })
    .sort((left, right) => left.time - right.time);
}
