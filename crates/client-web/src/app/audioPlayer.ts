/** Persistent audio transport. Neither the audio element nor its listeners belong to a page render. */
import { createIcons, icons } from 'lucide';
import {
  createPlaybackSession,
  deletePlaybackSession,
  getItem,
  getPlaylist,
  getSessionStreamUrl,
  getWebClientProfile,
  updatePlaybackProgress,
  savePlaylist,
  saveRadioPlaylist,
  updateMusicQueue,
  getMusicLyrics,
  getMusicRadio,
  ApiError,
  type MediaItemDetail,
  type MediaItemSummary,
  type PlaybackSession,
  type MusicQueueState,
} from '../api';
import {
  orderedAlbumTracks,
  renderTrackListeningStatus,
  playlistPlaybackButtonContent,
} from './music';
import {
  activeMusicPlayback,
  clearMusicPlayback,
  musicPlaybackItem,
  musicPlaybackPlaylist,
  setMusicPlayback,
  musicCardStatus,
} from './musicPlaybackState';
import { syncMusicPlaybackCards } from './musicPlaybackCards';
import { renderButtonContent } from './ui';
import { AudioQueue } from './audioQueue';
import { audiobookChapterTargets, orderedAudiobookParts } from './audiobooks';
import {
  renderAudioPlayer,
  formatAudioTime,
  currentLyricText,
  type AudioPlayerViewState,
} from './audioPlayerView';
import { state } from './state';
import { setElementHtml } from './domPatcher';
import { musicMediaMetadata, revealPanelEntry } from './audioPlayerSupport';

export const audioQueue = new AudioQueue();
let audio: HTMLAudioElement | undefined, controls: HTMLDivElement | undefined;
let session: PlaybackSession | undefined,
  current: MediaItemDetail | undefined,
  owner: number | undefined;
let generation = 0,
  baseMs = 0,
  initialSeekMs = 0,
  lastSaved = -1;
let writes = Promise.resolve();
let refreshOnSave = false;
let radioQueueId: number | undefined,
  playingKey: number | undefined,
  radioContext = false;
let revealPanel = false,
  focusPanel = false,
  lastLyric: HTMLElement | undefined,
  followLyrics = true;
let callbacks = {
  changed: () => {},
  navigate: (_path: string) => {},
  saved: async () => {},
};
const view: AudioPlayerViewState = {
  queue: audioQueue,
  loading: false,
  paused: true,
  volume: 1,
  muted: false,
  lyricsEnabled: true,
  lyricsLoading: false,
  saving: false,
};

export function configureAudioPlayer(options: typeof callbacks): void {
  callbacks = options;
}
export function musicIsActive(): boolean {
  return Boolean(current || audioQueue.current);
}
export async function toggleCurrentMusic(itemId?: number): Promise<boolean> {
  if (
    !audio ||
    !current ||
    (itemId !== undefined && current.id !== itemId) ||
    audio.ended
  )
    return false;
  if (audio.paused) await audio.play();
  else audio.pause();
  return true;
}

function ensurePlayer(): void {
  if (audio) return;
  const host = document.createElement('div');
  host.id = 'music-player-host';
  audio = document.createElement('audio');
  audio.id = 'music-audio';
  audio.preload = 'metadata';
  audio.preservesPitch = true;
  audio.hidden = true;
  controls = document.createElement('div');
  controls.id = 'music-player-controls';
  host.append(audio, controls);
  document.body.append(host);
  host.addEventListener(
    'error',
    (event) => {
      if (event.target instanceof HTMLImageElement) event.target.hidden = true;
    },
    true,
  );
  host.addEventListener('click', (event) => {
    const target = (event.target as Element).closest<HTMLElement>('button');
    if (!target) return;
    const action = target.dataset.musicAction;
    if (action) void act(action).catch(showError);
    if (target.dataset.musicOpenItem)
      callbacks.navigate(`/items/${target.dataset.musicOpenItem}`);
    if (target.dataset.audioChapter !== undefined)
      void playChapter(Number(target.dataset.audioChapter)).catch(showError);
    if (
      target.dataset.musicQueueKey &&
      audioQueue.select(Number(target.dataset.musicQueueKey))
    )
      void playCurrent().catch(showError);
    if (target.dataset.musicRemove) {
      audioQueue.remove(Number(target.dataset.musicRemove));
      persist();
      paint();
      callbacks.changed();
    }
    if (target.dataset.musicMove) {
      const [key, direction] = target.dataset.musicMove.split(':').map(Number);
      audioQueue.move(key, direction as -1 | 1);
      persist();
      paint();
      callbacks.changed();
    }
  });
  host.addEventListener('input', (event) => {
    const target = event.target as HTMLInputElement;
    if (target.matches('[data-music-volume]')) {
      audio!.volume = Number(target.value);
      audio!.muted = false;
    }
  });
  host.addEventListener('change', (event) => {
    const target = event.target as HTMLInputElement;
    if (target.matches('[data-audio-speed]')) {
      const rate = Number(target.value);
      if (rate >= 0.5 && rate <= 3) {
        audio!.playbackRate = rate;
        view.playbackRate = rate;
      }
    }
    if (target.matches('[data-music-seek]'))
      seek((durationMs() * Number(target.value)) / 1000);
  });
  host.addEventListener(
    'wheel',
    (event) => {
      const target = event.target as Element;
      if (target.closest('[data-music-volume], [data-music-action="mute"]')) {
        event.preventDefault();
        audio!.volume = Math.max(
          0,
          Math.min(
            1,
            (audio!.muted ? 0 : audio!.volume) +
              (event.deltaY < 0 ? 0.05 : -0.05),
          ),
        );
        audio!.muted = audio!.volume === 0;
      } else if (target.closest('[data-music-seek]')) {
        event.preventDefault();
        seek(positionMs() + (event.deltaY < 0 ? 10000 : -10000));
      } else if (target.closest('.music-lyrics-scroll')) followLyrics = false;
    },
    { passive: false },
  );
  host.addEventListener('submit', (event) => {
    if (!(event.target as Element).matches('[data-music-save]')) return;
    event.preventDefault();
    const value = new FormData(event.target as HTMLFormElement).get('name');
    const name = typeof value === 'string' ? value.trim() : '';
    if (!name || view.saving) return;
    const user = owner;
    view.saving = true;
    paint();
    persist();
    const existing = radioQueueId;
    void writes
      .then(() =>
        existing
          ? saveRadioPlaylist(existing, name)
          : savePlaylist(
              name,
              audioQueue.ordered.map((entry) => entry.item.id),
            ),
      )
      .then(async () => {
        if (user !== owner) return;
        view.notice = `Saved “${name}”. Find it in Playlists.`;
        await callbacks.saved();
      })
      .catch(showError)
      .finally(() => {
        view.saving = false;
        paint();
      });
  });
  host.addEventListener('keydown', (event) => {
    if ((event.target as Element).matches('input')) return;
    if (event.key === 'Escape' && view.panel) {
      event.preventDefault();
      closePanel();
    } else if (event.key === ' ' && event.target === host) {
      event.preventDefault();
      void act('play').catch(showError);
    }
  });
  audio.addEventListener('loadedmetadata', () => {
    if (initialSeekMs > 0) {
      audio!.currentTime = initialSeekMs / 1000;
      initialSeekMs = 0;
    }
    updateTimeline();
    persist();
  });
  for (const event of ['durationchange', 'ratechange'])
    audio.addEventListener(event, updateTimeline);
  audio.addEventListener('seeked', () => {
    updateTimeline();
    persist();
  });
  host.addEventListener(
    'touchmove',
    (event) => {
      if ((event.target as Element).closest('.music-lyrics-scroll'))
        followLyrics = false;
    },
    { passive: true },
  );
  audio.addEventListener('timeupdate', () => {
    updateTimeline();
    const seconds = Math.floor(positionMs() / 1000);
    if (seconds - lastSaved >= 10) {
      lastSaved = seconds;
      persist();
    }
  });
  audio.addEventListener('pause', () => {
    view.paused = true;
    persist();
    paint();
  });
  audio.addEventListener('play', () => {
    view.paused = false;
    view.error = undefined;
    paint();
  });
  audio.addEventListener('waiting', () => {
    view.loading = true;
    paint();
  });
  audio.addEventListener('playing', () => {
    view.loading = false;
    paint();
  });
  audio.addEventListener('canplay', () => {
    view.loading = false;
    paint();
  });
  audio.addEventListener('error', () =>
    showError(
      new Error('This audio file could not be played. Retry or choose another file.'),
    ),
  );
  audio.addEventListener('volumechange', () => {
    view.volume = audio!.volume;
    view.muted = audio!.muted;
    paint();
  });
  audio.addEventListener('ended', () => {
    updateTimeline();
    persist(true);
    if (audioQueue.advance(true)) void playCurrent(0, false).catch(showError);
    else {
      view.paused = true;
      paint();
      callbacks.changed();
    }
  });
  document.addEventListener('visibilitychange', () => {
    if (document.hidden) persist();
  });
  globalThis.addEventListener('pagehide', () => persist());
}

function durationMs(): number {
  return (
    current?.duration_ms ??
    (audio && Number.isFinite(audio.duration)
      ? Math.round(audio.duration * 1000) + baseMs
      : 0)
  );
}
function positionMs(): number {
  return baseMs + Math.round((audio?.currentTime ?? 0) * 1000);
}
function persist(completed = false): void {
  if (!current || !session || !audio) return;
  const item = current,
    duration = durationMs(),
    user = owner;
  completed ||= audio.ended;
  const payload = {
    position_ms: completed
      ? duration
      : Math.min(duration || Infinity, positionMs()),
    duration_ms: duration || undefined,
    completed,
    radio: radioContext,
  };
  const queueId = radioQueueId,
    index = Math.max(0, audioQueue.order.indexOf(playingKey ?? -1));
  const refresh = completed || refreshOnSave;
  refreshOnSave = false;
  const queue: MusicQueueState = {
    item_ids: audioQueue.ordered.map((entry) => entry.item.id),
    current_index: index,
    position_ms: payload.position_ms,
    completed:
      completed &&
      index === audioQueue.order.length - 1 &&
      audioQueue.repeat === 'off',
    shuffle: audioQueue.shuffle,
    repeat_mode: audioQueue.repeat,
  };
  if (
    queueId &&
    user === state.bootstrap?.current_user?.id &&
    state.selectedPlaylist?.playlist.id === queueId
  ) {
    const { item_ids: _ids, ...progress } = queue;
    state.selectedPlaylist = {
      ...state.selectedPlaylist,
      items: audioQueue.ordered.map((entry) => entry.item),
      playlist: { ...state.selectedPlaylist.playlist, ...progress },
    };
  }
  // Serialize writes so a delayed periodic update cannot overwrite a completed song.
  writes = writes
    .catch(() => {})
    .then(async () => {
      if (user !== state.bootstrap?.current_user?.id) return;
      await updatePlaybackProgress(item.id, payload);
      if (queueId && queue.item_ids.length)
        await updateMusicQueue(queueId, queue);
      if (refresh && user === owner) await callbacks.saved();
    })
    .catch((error) => {
      console.warn('Unable to save music progress', error);
    });
}
function showError(error: unknown): void {
  view.error =
    error instanceof Error ? error.message : 'Music operation failed.';
  view.loading = false;
  paint();
}
function paint(): void {
  if (!controls) return;
  view.item = current ?? audioQueue.current?.item;
  const name = controls.querySelector<HTMLInputElement>('[name="name"]')?.value;
  const scroll = controls.querySelector('.music-queue-list')?.scrollTop;
  setElementHtml(controls, renderAudioPlayer(view), { preserveDom: true });
  controls
    .querySelectorAll<HTMLImageElement>('.music-cover img')
    .forEach((image) => {
      if (image.complete && image.naturalWidth === 0) image.hidden = true;
    });
  if (name) {
    const input = controls.querySelector<HTMLInputElement>('[name="name"]');
    if (input) input.value = name;
  }
  const list = controls.querySelector('.music-queue-list');
  if (list && scroll != null) list.scrollTop = scroll;
  document.body.classList.toggle('has-music-player', Boolean(view.item));
  document.body.classList.toggle(
    'has-music-panel',
    Boolean(view.item && view.panel),
  );
  createIcons({ icons, root: controls });
  updateTimeline();
  if (revealPanel) {
    revealPanel = false;
    const focus = focusPanel;
    focusPanel = false;
    requestAnimationFrame(() => {
      if (focus)
        controls
          ?.querySelector<HTMLElement>('#music-panel-title')
          ?.focus({ preventScroll: true });
      if (view.panel === 'queue')
        revealPanelEntry(
          controls?.querySelector('.music-queue-list'),
          controls?.querySelector('.music-queue-entry.is-current'),
        );
      if (view.panel === 'lyrics')
        revealPanelEntry(
          controls?.querySelector('.music-lyrics-scroll'),
          controls?.querySelector('.music-lyrics .is-current'),
        );
    });
  }
}
function closePanel(): void {
  const previous = view.panel;
  view.panel = undefined;
  paint();
  if (previous)
    controls
      ?.querySelector<HTMLButtonElement>(`[data-music-action="${previous}"]`)
      ?.focus({ preventScroll: true });
}
function updateTimeline(): void {
  if (!controls || !audio) return;
  const duration = durationMs(),
    position = positionMs();
  setMusicPlayback(
    current
      ? {
          userId: owner,
          queueId: radioQueueId,
          queueIndex: audioQueue.order.indexOf(playingKey ?? -1),
          queueItems: radioQueueId
            ? audioQueue.ordered.map((entry) => entry.item)
            : undefined,
          item: current,
          positionMs: position,
          durationMs: duration,
          paused: audio.paused,
          ended: audio.ended,
        }
      : undefined,
  );
  syncVisibleMusicPlayback();
  const seekControl =
    controls.querySelector<HTMLInputElement>('[data-music-seek]');
  if (seekControl && document.activeElement !== seekControl)
    seekControl.value = String(
      duration > 0 ? Math.min(1000, (position / duration) * 1000) : 0,
    );
  for (const [selector, value] of [
    ['[data-music-elapsed]', position],
    ['[data-music-duration]', duration],
  ] as const) {
    const label = controls.querySelector(selector);
    if (label) label.textContent = formatAudioTime(value / 1000);
  }
  updateLyrics(position);
  updateChapters(position);
  updateMediaSession(duration, position);
}

/** Chapter starts can precede the current file when a chapter spans several files. */
function updateChapters(position: number): void {
  if (!controls || !current?.audiobook) return;
  const indices = new Map(audioQueue.ordered.map((entry, index) => [entry.item.id, index]));
  const currentIndex = indices.get(current.id) ?? -1;
  let selected = -1;
  for (const [index, chapter] of (view.chapters ?? []).entries()) {
    const fileIndex = indices.get(chapter.itemId) ?? Infinity;
    if (fileIndex < currentIndex || (fileIndex === currentIndex && chapter.startMs <= position)) selected = index;
  }
  controls.querySelectorAll<HTMLElement>('[data-audio-chapter]').forEach((button) => {
    const active = Number(button.dataset.audioChapter) === selected;
    button.classList.toggle('is-current', active);
    if (active) button.setAttribute('aria-current', 'true');
    else button.removeAttribute('aria-current');
  });
}

function updateLyrics(position: number): void {
  if (!controls) return;
  const lines = [
    ...controls.querySelectorAll<HTMLElement>('[data-lyric-time]'),
  ];
  let selected: HTMLElement | undefined;
  for (const line of lines) {
    if (Number(line.dataset.lyricTime) <= position / 1000) selected = line;
  }
  const activeText = currentLyricText(
    view.lyrics?.synced_lyrics ?? '',
    position / 1000,
  );
  lines.forEach((line) =>
    line.classList.toggle('is-current', line === selected),
  );
  const compact = controls.querySelector('[data-current-lyric]');
  if (compact) {
    compact.textContent = activeText;
    const row = compact.closest<HTMLElement>('.music-inline-lyrics');
    if (row) row.hidden = !activeText;
  }
  // Reserve a stable lyric line for timed songs, including instrumental gaps, without placeholders.
  document.body.classList.toggle(
    'has-music-lyrics',
    Boolean(view.item && view.lyricsEnabled && view.lyrics?.synced_lyrics),
  );
  if (selected !== lastLyric) {
    if (followLyrics && view.panel === 'lyrics')
      revealPanelEntry(
        controls.querySelector('.music-lyrics-scroll'),
        selected,
      );
    lastLyric = selected;
  }
}

function updateMediaSession(duration: number, position: number): void {
  if (!audio) return;
  if ('mediaSession' in navigator) {
    navigator.mediaSession.playbackState = audio.paused ? 'paused' : 'playing';
    if (Number.isFinite(duration) && duration > 0)
      try {
        navigator.mediaSession.setPositionState({
          duration: duration / 1000,
          position: Math.max(0, Math.min(duration, position)) / 1000,
          playbackRate: audio.playbackRate,
        });
      } catch {
        /* Some browsers omit position reporting. */
      }
  }
}

/** Update only playback widgets, leaving page focus, scrolling and audio untouched. */
function syncVisibleMusicPlayback(): void {
  syncMusicPlaybackCards();
  const playback = activeMusicPlayback();
  const playlist = state.selectedPlaylist
    ? musicPlaybackPlaylist(state.selectedPlaylist)
    : undefined;
  document
    .querySelectorAll<HTMLElement>('[data-music-track-id]')
    .forEach((row) => {
      const id = Number(row.dataset.musicTrackId);
      const queueId =
        row.dataset.musicPlaylistId === undefined
          ? undefined
          : Number(row.dataset.musicPlaylistId);
      const index = Number(row.dataset.musicPlaylistIndex);
      const saved = visibleTrack(id, queueId, index, playlist);
      if (!saved) return;
      const item = musicPlaybackItem(saved);
      const isCurrent = trackIsCurrent(id, queueId, index, playback);
      row.classList.toggle('is-current', isCurrent);
      if (isCurrent) row.setAttribute('aria-current', 'true');
      else row.removeAttribute('aria-current');
      const status = row.querySelector<HTMLElement>('.music-track-status');
      const markup = renderTrackListeningStatus(item, isCurrent);
      if (status && status.dataset.playbackStatus !== markup) {
        setElementHtml(status, markup, { preserveDom: true });
        status.dataset.playbackStatus = markup;
        createIcons({ icons, root: status });
      }
      const button = row.querySelector<HTMLButtonElement>(
        '[data-play-music-track-id], [data-play-playlist-track]',
      );
      const label = `${isCurrent && !playback?.paused ? 'Pause' : 'Play'} ${item.display_title}`;
      if (button && button.getAttribute('aria-label') !== label) {
        button.setAttribute('aria-label', label);
        setElementHtml(
          button,
          renderButtonContent(
            '',
            isCurrent && !playback?.paused ? 'pause' : 'play',
          ),
          { preserveDom: true },
        );
        createIcons({ icons, root: button });
      }
    });
  syncPlaylistButton(playlist);
  syncAlbumButton(playback);
}

function visibleTrack(
  id: number,
  queueId: number | undefined,
  index: number,
  playlist: typeof state.selectedPlaylist,
): MediaItemSummary | undefined {
  if (queueId === undefined)
    return state.selectedItem?.children.find((item) => item.id === id);
  if (playlist?.playlist.id === queueId) return playlist.items[index];
  return undefined;
}

function syncPlaylistButton(playlist: typeof state.selectedPlaylist): void {
  const playlistButton = document.querySelector<HTMLButtonElement>(
    '[data-play-playlist]',
  );
  if (
    playlistButton &&
    playlist?.playlist.id === Number(playlistButton.dataset.playPlaylist)
  ) {
    const content = playlistPlaybackButtonContent(playlist);
    if (playlistButton.dataset.playbackContent !== content) {
      setElementHtml(playlistButton, content, { preserveDom: true });
      playlistButton.dataset.playbackContent = content;
      createIcons({ icons, root: playlistButton });
    }
  }
}

function syncAlbumButton(
  playback: ReturnType<typeof activeMusicPlayback>,
): void {
  const button = document.querySelector<HTMLButtonElement>(
    '[data-toggle-music-album], [data-listen-book]',
  );
  if (!button || !playback || playback.ended) return;
  if (Number(button.dataset.listenBook ?? button.dataset.toggleMusicAlbum) !== playback.item.parent_id) return;
  if (button.dataset.playbackState === String(playback.paused)) return;
  const action = playback.paused ? 'Resume' : 'Pause';
  const kind = button.dataset.listenBook ? 'listening' : 'album';
  if (button.dataset.listenBook) button.title = `${action} listening · Hold for listening options`;
  setElementHtml(button, renderButtonContent(`${action} ${kind}`, playback.paused ? 'play' : 'pause', 'start', playback.item.display_title), { preserveDom: true });
  button.dataset.playbackState = String(playback.paused);
  createIcons({ icons, root: button });
}

async function playCurrent(startMs = 0, savePrevious = true): Promise<void> {
  const entry = audioQueue.current;
  if (!entry) return;
  ensurePlayer();
  if (savePrevious) persist();
  const token = ++generation,
    previous = session;
  session = undefined;
  current = undefined;
  audio!.pause();
  audio!.removeAttribute('src');
  audio!.load();
  view.error = undefined;
  view.loading = true;
  view.lyrics = undefined;
  view.lyricsLoading = false;
  view.lyricsError = undefined;
  paint();
  callbacks.changed();
  if (previous)
    void deletePlaybackSession(previous.session_id).catch(console.warn);
  const item = await getItem(entry.item.id);
  if (token !== generation) return;
  if (!item.playable || item.missing_since || item.media_kind !== 'audio')
    throw new Error('This audio file is no longer available.');
  const created = await createPlaybackSession({
    item_id: item.id,
    client_profile: getWebClientProfile(),
  });
  if (token !== generation) {
    await deletePlaybackSession(created.session_id);
    return;
  }
  current = item;
  session = created;
  playingKey = entry.key;
  lastSaved = Math.floor(startMs / 1000);
  lastLyric = undefined;
  followLyrics = true;
  revealPanel = true;
  refreshOnSave = true;
  baseMs = created.decision.transcode_required ? startMs : 0;
  initialSeekMs = created.decision.transcode_required ? 0 : startMs;
  audio!.src = getSessionStreamUrl(created.session_id, baseMs);
  bindMediaSession();
  paint();
  callbacks.changed();
  audio!.playbackRate = item.audiobook ? (view.playbackRate ?? 1) : 1;
  if (view.lyricsEnabled && !item.audiobook) void loadLyrics();
  try {
    await audio!.play();
  } catch {
    view.paused = true;
    view.loading = false;
    view.error = 'Playback is paused. Press Play to start audio.';
    paint();
  }
}

/** Replace the audio queue, optionally retaining audiobook chapter navigation. */
export async function playMusicItems(
  items: MediaItemSummary[],
  start = 0,
  startMs = 0,
  book?: MediaItemDetail,
): Promise<void> {
  if (state.bootstrap?.current_user?.id === undefined)
    throw new Error('Sign in to play music.');
  ensurePlayer();
  persist();
  radioQueueId = undefined;
  radioContext = false;
  view.queueName = undefined;
  view.chapters = book ? audiobookChapterTargets(book) : undefined;
  view.panel = undefined;
  if (book) {
    audioQueue.shuffle = false;
    audioQueue.repeat = 'off';
  }
  owner = state.bootstrap?.current_user?.id;
  audioQueue.replace(
    items.filter(
      (item) =>
        item.playable && !item.missing_since && item.media_kind === 'audio',
    ),
    start,
  );
  if (!audioQueue.current) throw new Error('No available songs to play.');
  await playCurrent(startMs, false);
}

/** Resume an audiobook in file order; chapter seeking preserves the existing transport. */
export async function playAudiobookItem(item: MediaItemDetail, startMs = 0): Promise<void> {
  const user = state.bootstrap?.current_user?.id;
  if (user === undefined) throw new Error('Sign in to listen to audiobooks.');
  if (owner === user && current?.id === item.id && audio && !audio.ended) {
    seek(startMs);
    await audio.play();
    return;
  }
  const token = ++generation;
  let book = item.item_type === 'audiobook' ? item : undefined;
  if (!book && item.parent_id) book = await getItem(item.parent_id);
  if (token !== generation || user !== state.bootstrap?.current_user?.id) return;
  if (book?.item_type !== 'audiobook') throw new Error('This audiobook is unavailable.');
  const parts = orderedAudiobookParts(book.children).filter((part) => part.playable && !part.missing_since);
  const start = item.item_type === 'audiobook_part' ? parts.findIndex((part) => part.id === item.id) : 0;
  if (start < 0) throw new Error('This audio file is unavailable.');
  await playMusicItems(parts, start, startMs, book);
}

async function playChapter(index: number): Promise<void> {
  const chapter = view.chapters?.[index];
  if (!chapter || !audio) return;
  if (chapter.itemId === current?.id) {
    seek(chapter.startMs);
    await audio.play();
    return;
  }
  const entry = audioQueue.entries.find((entry) => entry.item.id === chapter.itemId);
  if (entry && audioQueue.select(entry.key)) await playCurrent(chapter.startMs);
}
export async function playMusicItem(
  item: MediaItemDetail,
  startMs = 0,
): Promise<void> {
  const token = ++generation,
    user = state.bootstrap?.current_user?.id;
  if (user === undefined) throw new Error('Sign in to play music.');
  const album = await albumForItem(item);
  if (token !== generation || user !== state.bootstrap?.current_user?.id)
    return;
  const tracks =
    album?.item_type === 'album'
      ? orderedAlbumTracks(album.children).filter(
          (track) => track.playable && !track.missing_since,
        )
      : [item];
  const start =
    item.item_type === 'track'
      ? tracks.findIndex((track) => track.id === item.id)
      : 0;
  if (start < 0) throw new Error('The selected song is unavailable.');
  await playMusicItems(tracks, start, startMs);
}
export async function queueMusicItem(id: number, next = false): Promise<void> {
  const token = generation,
    item = await getItem(id);
  if (token !== generation) return;
  const items =
    item.item_type === 'album' ? orderedAlbumTracks(item.children) : [item];
  const available = items.filter(
    (track) =>
      track.playable && !track.missing_since && track.media_kind === 'audio',
  );
  if (!musicIsActive() || current?.audiobook || audioQueue.current?.item.audiobook) {
    await playMusicItems(available);
    return;
  }
  audioQueue.enqueue(available, next);
  view.notice = next ? 'Added to play next.' : 'Added to queue.';
  paint();
  persist();
  callbacks.changed();
}
export function stopMusic(): void {
  ++generation;
  if (!musicIsActive() && !session) return;
  persist();
  const previous = session;
  session = undefined;
  current = undefined;
  audioQueue.replace([]);
  view.item = undefined;
  view.panel = undefined;
  view.notice = undefined;
  view.chapters = undefined;
  view.playbackRate = undefined;
  radioQueueId = undefined;
  radioContext = false;
  playingKey = undefined;
  setMusicPlayback(undefined);
  if (audio) {
    audio.pause();
    audio.removeAttribute('src');
    audio.load();
  }
  paint();
  callbacks.changed();
  if (previous)
    void deletePlaybackSession(previous.session_id).catch(console.warn);
  if ('mediaSession' in navigator) {
    navigator.mediaSession.metadata = null;
    navigator.mediaSession.playbackState = 'none';
    try {
      navigator.mediaSession.setPositionState();
    } catch {
      /* Optional API. */
    }
    for (const action of [
      'play',
      'pause',
      'nexttrack',
      'previoustrack',
      'seekbackward',
      'seekforward',
      'seekto',
      'stop',
    ] as MediaSessionAction[])
      try {
        navigator.mediaSession.setActionHandler(action, null);
      } catch {
        /* Optional actions. */
      }
  }
}
/** Called after page paints only to enforce account boundaries; it never replaces the audio node. */
export function syncAudioPlayer(): void {
  if (state.bootstrap?.current_user?.id !== owner) {
    clearMusicPlayback();
    if (musicIsActive()) stopMusic();
  }
}
export async function startMusicRadio(id: number): Promise<void> {
  const token = generation,
    user = state.bootstrap?.current_user?.id;
  const [radio, seed] = await Promise.all([getMusicRadio(id), getItem(id)]);
  if (token !== generation || user !== state.bootstrap?.current_user?.id)
    return;
  if (!radio.items.length)
    throw new Error(
      radio.message ?? 'No similar songs are available in your libraries.',
    );
  const saved = await savePlaylist(
    `${seed.display_title.slice(0, 190)} radio`,
    radio.items.map((item) => item.id),
    true,
    id,
  );
  if (token !== generation || user !== state.bootstrap?.current_user?.id)
    return;
  await resumeMusicQueue(saved.playlist.id);
  view.notice = radio.message ?? `Radio from ${radio.source}`;
  view.panel = 'queue';
  paint();
}
export async function resumeMusicQueue(
  id: number,
  activePath = `/items/playlists/${id}`,
): Promise<void> {
  const user = state.bootstrap?.current_user?.id;
  if (user === undefined) throw new Error('Sign in to resume music.');
  if (
    activeMusicPlayback()?.userId === user &&
    musicCardStatus({ queueId: id })
  ) {
    callbacks.navigate(activePath);
    return;
  }
  await playMusicPlaylist(id);
}

/** Play, resume, or select an occurrence without discarding the active queue's order. */
export async function playMusicPlaylist(
  id: number,
  selectedIndex?: number,
): Promise<void> {
  const user = state.bootstrap?.current_user?.id;
  if (user === undefined) throw new Error('Sign in to play music.');
  if (
    selectedIndex !== undefined &&
    (!Number.isInteger(selectedIndex) || selectedIndex < 0)
  )
    throw new Error('This audio file is no longer available.');
  const active = activeMusicPlayback();
  if (await selectActivePlaylist(id, selectedIndex, user)) return;
  const token = ++generation;
  await writes;
  if (token !== generation || user !== state.bootstrap?.current_user?.id)
    return;
  const detail = await getPlaylist(id);
  if (token !== generation || user !== state.bootstrap?.current_user?.id)
    return;
  const available = (item: MediaItemSummary) =>
    item.playable && !item.missing_since && item.media_kind === 'audio';
  const completed =
    detail.playlist.completed || (active?.queueId === id && active.ended);
  const requested =
    selectedIndex ?? (completed ? 0 : (detail.playlist.current_index ?? 0));
  if (
    selectedIndex !== undefined &&
    (!detail.items[requested] || !available(detail.items[requested]))
  )
    throw new Error('This audio file is no longer available.');
  const items = detail.items.filter(available);
  if (!items.length) throw new Error('No available songs in this playlist.');
  const start = detail.items.slice(0, requested).filter(available).length;
  const startMs =
    selectedIndex === undefined &&
    !completed &&
    detail.items[requested] &&
    available(detail.items[requested])
      ? (detail.playlist.position_ms ?? 0)
      : 0;
  persist();
  ensurePlayer();
  owner = user;
  radioQueueId = id;
  radioContext = Boolean(
    detail.playlist.temporary || detail.playlist.artwork_item_id,
  );
  audioQueue.shuffle = false;
  audioQueue.replace(items, start);
  audioQueue.shuffle = detail.playlist.shuffle ?? false;
  audioQueue.repeat = detail.playlist.repeat_mode ?? 'off';
  view.queueName = detail.playlist.name;
  view.chapters = undefined;
  if (view.panel === 'chapters') view.panel = undefined;
  await playCurrent(startMs, false);
  void callbacks.saved();
}
async function loadLyrics(): Promise<void> {
  if (!current) return;
  const id = current.id,
    token = generation;
  if (view.lyricsLoading) return;
  view.lyricsLoading = true;
  view.lyricsError = undefined;
  paint();
  try {
    const lyrics = await getMusicLyrics(id);
    if (token === generation) {
      view.lyrics = lyrics;
      revealPanel = true;
    }
  } catch (error) {
    if (token === generation) {
      view.lyricsError =
        error instanceof ApiError && error.status === 404
          ? 'This audio file is no longer available.'
          : 'Saved lyrics could not be loaded.';
    }
  } finally {
    if (token === generation) {
      view.lyricsLoading = false;
      paint();
    }
  }
}
function seek(position: number): void {
  if (!audio || !session) return;
  const target = Math.max(0, Math.min(durationMs() || Infinity, position));
  if (session.decision.transcode_required) {
    const playing = !audio.paused;
    baseMs = target;
    initialSeekMs = 0;
    audio.src = getSessionStreamUrl(session.session_id, target);
    if (playing) void audio.play().catch(showError);
  } else audio.currentTime = target / 1000;
  updateTimeline();
}
function togglePanel(panel: 'queue' | 'lyrics' | 'chapters'): void {
  if (view.panel === panel) {
    closePanel();
    return;
  }
  view.panel = panel;
  revealPanel = true;
  focusPanel = true;
  if (panel === 'lyrics') {
    view.lyricsEnabled = true;
    followLyrics = true;
    if (!view.lyrics) {
      void loadLyrics();
      return;
    }
  }
  paint();
}
async function previousTrack(): Promise<void> {
  if (positionMs() > 3000 || !audioQueue.previous()) seek(0);
  else await playCurrent();
}
async function act(action: string): Promise<void> {
  if (!audio) return;
  const actions: Record<string, () => void | Promise<unknown>> = {
    play: async () => {
      if (audio!.paused) await audio!.play();
      else audio!.pause();
    },
    next: async () => {
      if (audioQueue.advance()) await playCurrent();
    },
    previous: previousTrack,
    back: () => seek(positionMs() - 10000),
    forward: () => seek(positionMs() + 10000),
    shuffle: () => {
      audioQueue.toggleShuffle();
      persist();
      paint();
      callbacks.changed();
    },
    repeat: () => {
      audioQueue.repeat = ({ off: 'all', all: 'one', one: 'off' } as const)[
        audioQueue.repeat
      ];
      persist();
      paint();
    },
    mute: () => {
      audio!.muted = !audio!.muted;
    },
    queue: () => togglePanel('queue'),
    lyrics: () => togglePanel('lyrics'),
    chapters: () => togglePanel('chapters'),
    'follow-lyrics': () => {
      followLyrics = true;
      lastLyric = undefined;
      updateTimeline();
    },
    'close-panel': closePanel,
    clear: () => {
      audioQueue.clearUpcoming();
      persist();
      paint();
      callbacks.changed();
    },
    retry: () => playCurrent(positionMs()),
    stop: stopMusic,
  };
  await actions[action]?.();
}

function bindMediaSession(): void {
  if (!('mediaSession' in navigator) || !current) return;
  if ('MediaMetadata' in globalThis)
    navigator.mediaSession.metadata = new MediaMetadata(
      musicMediaMetadata(current),
    );
  const actions: Partial<
    Record<MediaSessionAction, MediaSessionActionHandler>
  > = {
    play: () => {
      if (audio?.paused) void audio.play().catch(showError);
    },
    pause: () => audio?.pause(),
    nexttrack: () => {
      void act('next').catch(showError);
    },
    previoustrack: () => {
      void act('previous').catch(showError);
    },
    seekbackward: (details) =>
      seek(positionMs() - (details.seekOffset ?? 10) * 1000),
    seekforward: (details) =>
      seek(positionMs() + (details.seekOffset ?? 10) * 1000),
    seekto: (details) => {
      if (details.seekTime != null) seek(details.seekTime * 1000);
    },
    stop: stopMusic,
  };
  for (const [action, handler] of Object.entries(actions)) {
    try {
      navigator.mediaSession.setActionHandler(
        action as MediaSessionAction,
        handler!,
      );
    } catch {
      /* Optional OS controls. */
    }
  }
}

async function albumForItem(
  item: MediaItemDetail,
): Promise<MediaItemDetail | undefined> {
  if (item.item_type === 'album') return item;
  return item.parent_id ? getItem(item.parent_id) : undefined;
}

async function selectActivePlaylist(
  id: number,
  selectedIndex: number | undefined,
  user: number,
): Promise<boolean> {
  const active = activeMusicPlayback();
  if (active?.userId !== user || active.queueId !== id || active.ended)
    return false;
  if (selectedIndex === undefined) {
    await toggleCurrentMusic();
    return true;
  }
  const entry = audioQueue.ordered[selectedIndex];
  if (!entry) throw new Error('This audio file is no longer available.');
  if (entry.key === playingKey) {
    await toggleCurrentMusic();
    return true;
  }
  audioQueue.select(entry.key);
  await playCurrent();
  return true;
}

function trackIsCurrent(
  id: number,
  queueId: number | undefined,
  index: number,
  playback: ReturnType<typeof activeMusicPlayback>,
): boolean {
  if (!playback || playback.ended) return false;
  if (queueId === undefined) return playback.item.id === id;
  return playback.queueId === queueId && playback.queueIndex === index;
}
