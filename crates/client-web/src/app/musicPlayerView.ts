import { getArtworkUrl, type MediaItemSummary, type MusicLyrics } from '../api';
import { escapeHtml } from './format';
import { renderIcon } from './ui';
import type { AppIconName } from './types';
import type { MusicQueue } from './musicQueue';

export interface MusicPlayerViewState {
  queue: MusicQueue; item?: MediaItemSummary; loading: boolean; paused: boolean; error?: string;
  volume: number; muted: boolean; panel?: 'queue' | 'lyrics'; lyrics?: MusicLyrics; lyricsLoading: boolean;
  lyricsError?: string; notice?: string; saving: boolean;
  lyricsEnabled?:boolean; queueName?:string;
}
export function formatMusicTime(seconds: number): string {
  const value = Number.isFinite(seconds) ? Math.max(0, Math.floor(seconds)) : 0;
  const minutes = Math.floor(value / 60), remainder = String(value % 60).padStart(2, '0');
  return `${minutes}:${remainder}`;
}
function button(action: string, label: string, icon: AppIconName, active = false, disabled = false): string {
  return `<button type="button" class="player-icon-button music-control ${active ? 'is-active' : ''}" data-music-action="${action}" aria-label="${escapeHtml(label)}" title="${escapeHtml(label)}" ${['shuffle','repeat','queue','lyrics'].includes(action) ? `aria-pressed="${active}"` : ''} ${['queue','lyrics'].includes(action) ? `aria-controls="music-panel" aria-expanded="${active}"` : ''} ${disabled ? 'disabled' : ''}>${renderIcon(icon)}</button>`;
}
export function renderMusicPlayer(view: MusicPlayerViewState): string {
  const { queue, item } = view;
  if (!item) return '';
  const title = item.display_title, artist = item.music?.artist ?? item.display_subtitle ?? '';
  const repeatLabel = { off: 'Repeat off', all: 'Repeat all', one: 'Repeat one' }[queue.repeat];
  return `${view.panel ? renderPanel(view) : ''}<section id="music-dock" class="music-dock" aria-label="Music player" data-nav-section="music-player">
    <div class="music-now-playing"><button class="music-cover" type="button" data-music-open-item="${item.parent_id ?? item.id}" aria-label="Open album">${renderIcon('music')}<img src="${escapeHtml(getArtworkUrl(item.artwork_item_id ?? item.parent_id ?? item.id, 'poster', item.artwork_updated_at))}" alt="" /></button>
      <div><button class="music-title" type="button" data-music-open-item="${item.id}">${escapeHtml(title)}</button><span class="muted">${escapeHtml(artist)}</span></div></div>
    <div class="music-transport"><div class="music-buttons">
      ${button('shuffle', 'Shuffle', 'shuffle', queue.shuffle)}${button('previous', 'Previous song', 'skip-back')}${button('back', 'Back 10 seconds', 'rotate-ccw')}
      ${button('play', view.paused ? 'Play music' : 'Pause music', view.loading ? 'refresh-cw' : view.paused ? 'play' : 'pause', false, view.loading)}
      ${button('forward', 'Forward 10 seconds', 'rotate-ccw')}${button('next', 'Next song', 'skip-forward', false, queue.cursor >= queue.order.length - 1 && queue.repeat !== 'all')}
      ${button('repeat', repeatLabel, queue.repeat === 'one' ? 'repeat-1' : 'repeat', queue.repeat !== 'off')}
      </div><div class="music-timeline"><span data-music-elapsed>0:00</span><input class="player-progress" data-music-seek type="range" min="0" max="1000" step="1" value="0" aria-label="Music position"/><span data-music-duration>${formatMusicTime((item.duration_ms ?? 0) / 1000)}</span></div></div>
    <div class="music-tools">${button('lyrics', 'Lyrics', 'mic-2', view.panel === 'lyrics')}${button('queue', 'Playing queue', 'list-music', view.panel === 'queue')}
      ${button('mute', view.muted ? 'Unmute music' : 'Mute music', view.muted ? 'volume-x' : 'volume-2')}
      <input class="player-volume" data-music-volume type="range" min="0" max="1" step="0.01" value="${view.muted ? 0 : view.volume}" aria-label="Music volume"/>
      ${button('stop', 'Stop music', 'x')}</div>
    ${view.lyricsEnabled ? `<div class="music-inline-lyrics" hidden><button type="button" data-music-action="lyrics" data-current-lyric aria-label="Expand current lyrics"></button></div>` : ''}
    ${view.error ? `<div class="music-feedback" role="alert">${escapeHtml(view.error)} ${button('retry', 'Retry playback', 'refresh-cw')}</div>` : ''}
  </section>`;
}
function renderPanel(view: MusicPlayerViewState): string {
  const { queue } = view;
  return `<section id="music-panel" class="music-panel" data-nav-section="music-panel" aria-label="${view.panel === 'queue' ? 'Playing queue' : 'Lyrics'}"><header><div><h3 id="music-panel-title" tabindex="-1">${view.panel === 'queue' ? 'Playing queue' : 'Lyrics'}</h3><p class="muted">${escapeHtml(view.panel==='queue' ? view.queueName ?? `${queue.order.length} songs` : view.item?.display_title ?? '')}</p></div>${button('close-panel','Close music panel','x')}</header>
    ${view.panel === 'lyrics' ? `<div class="music-lyrics-scroll">${renderLyrics(view)}</div><footer>${button('follow-lyrics','Jump to current lyrics','locate-fixed')}</footer>` : `<div class="music-queue-list">${queue.ordered.map((entry, index) => `<div class="music-queue-entry ${entry.key === queue.current?.key ? 'is-current' : ''}">
      <button type="button" data-music-queue-key="${entry.key}" ${entry.key === queue.current?.key ? 'aria-current="true"' : ''}><span>${index + 1}</span><span><strong>${escapeHtml(entry.item.display_title)}</strong><small>${escapeHtml(entry.item.music?.artist ?? '')}</small></span></button>
      ${index > queue.cursor ? `<button type="button" class="music-control" data-music-move="${entry.key}:-1" aria-label="Move song up">${renderIcon('chevron-up')}</button><button type="button" class="music-control" data-music-move="${entry.key}:1" aria-label="Move song down">${renderIcon('chevron-down')}</button>` : ''}
      ${entry.key !== queue.current?.key ? `<button type="button" class="music-control" data-music-remove="${entry.key}" aria-label="Remove ${escapeHtml(entry.item.display_title)}">${renderIcon('x')}</button>` : ''}</div>`).join('')}</div>
      <footer><button type="button" class="secondary-button" data-music-action="clear">Clear upcoming</button><form data-music-save><label>Playlist name<input id="music-playlist-name" name="name" required maxlength="200" autocomplete="off" placeholder="My playlist"/></label><button type="submit" ${view.saving ? 'disabled' : ''}>${view.saving ? 'Saving…' : 'Save as playlist'}</button></form></footer>`}
    ${view.notice && view.panel === 'queue' ? `<p class="music-feedback" role="status">${escapeHtml(view.notice)}</p>` : ''}
  </section>`;
}
function renderLyrics(view: MusicPlayerViewState): string {
  if (view.lyricsLoading) return '<p role="status">Looking up lyrics…</p>';
  if (view.lyricsError) return `<p role="status">${escapeHtml(view.lyricsError)}</p><button type="button" class="secondary-button" data-music-action="retry-lyrics">Try lyrics again</button>`;
  if (!view.lyrics) return '<p>No lyrics have been requested.</p>';
  const lines = parseTimedLyrics(view.lyrics.synced_lyrics ?? '');
  const text = view.lyrics.instrumental ? '<p>Instrumental</p>' : lines.length ? `<div class="music-lyrics">${lines.map((line) => `<p data-lyric-time="${line.time}">${escapeHtml(line.text)}</p>`).join('')}</div>` : `<pre class="music-lyrics">${escapeHtml(view.lyrics.plain_lyrics ?? 'No lyrics are available for this song.')}</pre>`;
  return `${text}<p class="muted">Lyrics from <a href="${escapeHtml(view.lyrics.source_url)}" target="_blank" rel="noreferrer">${escapeHtml(view.lyrics.source)}</a></p>`;
}
/** Empty timestamped lines clear the inline lyric during instrumental gaps. */
export function currentLyricText(text: string, seconds: number): string {
  return parseTimedLyrics(text).filter((line) => line.time <= seconds).pop()?.text ?? '';
}

/** Support multiple timestamps per line, decimal seconds, and LRC offset metadata. */
export function parseTimedLyrics(text: string): Array<{ time: number; text: string }> {
  const offset = Number(/\[offset:([+-]?\d+)\]/i.exec(text)?.[1] ?? 0) / 1000;
  return text.split(/\r?\n/).flatMap((line) => {
    const matches = [...line.matchAll(/\[(\d+):(\d+(?:\.\d+)?)\]/g)];
    const content = line.replace(/\[[^\]]*\]/g, '').trim();
    return matches.map((match) => ({ time: Math.max(0, Number(match[1]) * 60 + Number(match[2]) + offset), text: content }));
  }).sort((left, right) => left.time - right.time);
}
