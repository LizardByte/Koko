/** Controls trailer, theme-song, and browser playback UI state. */

import { playAudiobookItem, playMusicItem, playPodcastItem, musicIsActive, stopMusic } from './audioPlayer';
import { createIcons, icons } from 'lucide';
import type { AppIconName, ThemeSongSource, TrailerOption, YouTubePlayer } from './types';
import type { MediaAudioTrack, MediaItemDetail, PlaybackSession } from '../api';
import {
  createPlaybackSession,
  deletePlaybackSession,
  getArtworkUrl,
  getItem,
  getSessionStreamUrl,
  getWebClientProfile,
  getPlaybackSessionStatus,
  finishPlaybackOnExit,
  resolveApiUrl,
  updatePlaybackProgress,
} from '../api';
import { PLAYBACK_QUALITIES, mediaPlaybackProblem, playbackDeliveryLabel, playbackProfile, playbackRequestProblem, renderPlaybackProblem, renderPlaybackQuality, type PlaybackQuality, type PlaybackProblem } from './playbackOptions';
import { YOUTUBE_PLAYER_STATE, YOUTUBE_THEME_PLACEHOLDER_VIDEO_ID } from './constants';
import { escapeHtml, formatDuration, formatMediaTime } from './format';
import { currentThemeSongTarget } from './mediaTargets';
import { state } from './state';
import { audiobookRestartTarget, type AudiobookRestartScope } from './audiobooks';
import { renderButtonContent, renderIcon, subtitleLanguage } from './ui';
import { buildYouTubeWatchUrl, extractYouTubeVideoId, loadYouTubeIframeApi } from './youtube';

type RenderCallback = (preserveScroll?: boolean) => void;
type RefreshDataCallback = (showLoading?: boolean) => Promise<void>;

let playbackRequest = 0;
const compatiblePlaybackSources = new Set<string>();
const reportedPlaybackErrors = new WeakMap<HTMLMediaElement, string | null>();
const initializedMediaSources = new WeakMap<HTMLMediaElement, {
  source: string; seekApplied: boolean; settingsApplied: boolean; started: boolean; cleanup?: () => void;
}>();

let renderApp: RenderCallback = () => undefined;
let refreshAppData: RefreshDataCallback = () => Promise.resolve();

/** Connects playback side effects back to the application coordinator. */
export function configurePlaybackController(callbacks: { render: RenderCallback; refreshData: RefreshDataCallback }): void {
  renderApp = callbacks.render;
  refreshAppData = callbacks.refreshData;
  globalThis.addEventListener('pagehide', finishActivePlaybackOnExit);
}

function render(preserveScroll = true): void {
  renderApp(preserveScroll);
}

function refreshData(showLoading = true): Promise<void> {
  return refreshAppData(showLoading);
}

let themeSongYouTubePlayer: YouTubePlayer | undefined;

let themeSongYouTubePlayerReady: Promise<YouTubePlayer> | undefined;

let activeThemeSongYouTubeVideoId: string | undefined;

let trailerYouTubePlayer: YouTubePlayer | undefined;

let trailerYouTubePlayerReady: Promise<YouTubePlayer> | undefined;

let activeTrailerYouTubeVideoId: string | undefined;

let trailerProgressHandle: number | undefined;

let trailerVolume = 1;

let trailerMuted = false;

const ESCALATING_SEEK_STEPS = [10, 20, 30, 60, 120, 300] as const;
const ESCALATING_SEEK_WINDOW_MS = 900;

/** Renders the active playback overlay, including trailers and browser playback. */
export function renderPlayerOverlay(): string {
  return state.activeTrailer ? renderTrailerOverlay() : renderMediaPlayerOverlay();
}

function renderTrailerControlsMarkup(videoId: string | undefined): string {
  if (!videoId) {
    return '';
  }

  return `
            <div class="player-bottom-controls player-controls">
              <input id="trailer-progress" class="player-progress" type="range" min="0" max="1000" value="0" step="1" aria-label="Trailer position" />
              <div class="player-control-row">
                <div class="player-control-cluster player-time-cluster">
                  <span class="player-time"><span id="trailer-current-time">0:00</span><span>/</span><span id="trailer-duration">0:00</span></span>
                </div>
                <div class="player-control-cluster player-transport-cluster">
                  <button class="player-icon-button" type="button" data-trailer-seek="-10" title="Back 10 seconds" aria-label="Back 10 seconds">${renderIcon('skip-back', 'player-control-icon')}</button>
                  <button id="trailer-play-toggle-small" class="player-icon-button player-primary-button" type="button" title="Pause" aria-label="Pause">${renderIcon('pause', 'player-control-icon')}</button>
                  <button class="player-icon-button" type="button" data-trailer-seek="10" title="Forward 10 seconds" aria-label="Forward 10 seconds">${renderIcon('skip-forward', 'player-control-icon')}</button>
                </div>
                <div class="player-control-cluster player-tool-cluster">
                  <button id="trailer-mute-toggle" class="player-icon-button" type="button" title="Mute" aria-label="Mute">${renderIcon('volume-2', 'player-control-icon')}</button>
                  <input id="trailer-volume" class="player-volume" type="range" min="0" max="1" value="${trailerMuted ? '0' : String(trailerVolume)}" step="0.01" aria-label="Trailer volume" />
                  <button id="trailer-fullscreen" class="player-icon-button" type="button" title="Fullscreen" aria-label="Fullscreen">${renderIcon('maximize', 'player-control-icon')}</button>
                </div>
              </div>
            </div>
          `;
}

function renderTrailerFrameMarkup(videoId: string | undefined): string {
  return videoId
    ? '<div id="trailer-player" class="trailer-youtube-player"></div>'
    : '<div class="trailer-unavailable">This external media URL is not a controllable YouTube video.</div>';
}

function renderTrailerTitleMarkup(
  itemLogoUrl: string | undefined,
  itemTitle: string | undefined,
  trailerTitle: string,
  brandedTrailerTitle: string,
): string {
  if (itemLogoUrl) {
    return `
                <div class="trailer-title-brand-row">
                  <img class="player-title-logo trailer-title-logo" src="${escapeHtml(itemLogoUrl)}" alt="${escapeHtml(itemTitle ?? 'Item logo')}" />
                  <h2>${escapeHtml(brandedTrailerTitle)}</h2>
                </div>
              `;
  }

  return `<h2>${escapeHtml(trailerTitle)}</h2>`;
}

function renderTrailerOverlay(): string {
  const activeTrailer = state.activeTrailer;
  if (!activeTrailer) {
    return '';
  }

  const videoId = extractYouTubeVideoId(activeTrailer.url);
  const watchUrl = buildYouTubeWatchUrl(activeTrailer.url);
  const label = activeTrailer.label ?? 'Trailer';
  const externalUrl = watchUrl ?? activeTrailer.url;
  const externalLinkLabel = watchUrl ? 'Open on YouTube' : 'Open Source';
  const errorHint = watchUrl ? 'Open it on YouTube or try again in a moment.' : 'Open the source link or try another extra.';
  const itemTitle = state.selectedItem?.display_title.trim();
  const itemLogoUrl = state.selectedItem?.logo_url ? resolveApiUrl(state.selectedItem.logo_url) : undefined;
  const trailerTitle = itemLogoUrl || !itemTitle
    ? activeTrailer.title
    : `${itemTitle} | ${activeTrailer.title}`;
  const trailerControlsMarkup = renderTrailerControlsMarkup(videoId);
  return `
      <div class="player-overlay trailer-overlay">
        <div class="player-shell trailer-shell is-controls-visible" tabindex="-1" ${videoId ? `data-trailer-video-id="${escapeHtml(videoId)}"` : ''}>
          <div class="trailer-frame-shell" aria-label="${escapeHtml(activeTrailer.title)}">
            ${renderTrailerFrameMarkup(videoId)}
          </div>
          <div class="trailer-youtube-chrome-mask" aria-hidden="true"></div>
          <div class="player-loading-indicator" aria-live="polite">
            <span class="loading-spinner player-loading-spinner" aria-hidden="true"></span>
          </div>
          <div class="player-error-indicator" aria-live="polite">
            <strong>${escapeHtml(label)} could not start</strong>
            <span>${escapeHtml(errorHint)}</span>
          </div>
          <div class="player-idle-hit-area trailer-idle-hit-area" aria-hidden="true"></div>
          <div class="player-top-controls player-controls trailer-top-controls">
            <div class="player-title-block">
              <span class="eyebrow">${escapeHtml(label)}</span>
              ${renderTrailerTitleMarkup(itemLogoUrl, itemTitle, trailerTitle, activeTrailer.title)}
            </div>
            <div class="player-top-actions">
              ${externalUrl ? `<a class="button-link secondary-button" href="${escapeHtml(externalUrl)}" target="_blank" rel="noreferrer">${renderButtonContent(externalLinkLabel, 'arrow-right')}</a>` : ''}
              <button id="close-trailer" class="player-icon-button" type="button" title="Close trailer" aria-label="Close trailer">${renderIcon('x', 'player-control-icon')}</button>
            </div>
          </div>
          ${trailerControlsMarkup}
        </div>
      </div>
    `;
}

function renderSubtitleTrackMarkup(playbackItem: MediaItemDetail, isAudio: boolean): string {
  if (isAudio) {
    return '';
  }

  return playbackItem.subtitle_tracks
    .map((track) => `<track kind="subtitles" label="${escapeHtml(track.label)}" srclang="${escapeHtml(subtitleLanguage(track.label))}" src="${escapeHtml(resolveApiUrl(track.url))}" />`)
    .join('');
}

function renderMediaElementMarkup(isAudio: boolean, source: string, posterUrl: string | undefined, trackMarkup: string): string {
  if (!isAudio) {
    return `
          <video id="media-player" autoplay preload="metadata" playsinline src="${escapeHtml(source)}">${trackMarkup}</video>
        `;
  }

  const audioArtMarkup = posterUrl
    ? `<img src="${escapeHtml(posterUrl)}" alt="" />`
    : renderIcon('music', 'audio-player-art-icon');
  const audioArtClass = posterUrl ? 'has-image' : '';
  return `
          <div class="audio-player-backdrop" aria-hidden="true"></div>
          <div class="audio-player-art ${audioArtClass}">
            ${audioArtMarkup}
          </div>
          <audio id="media-player" autoplay preload="metadata" src="${escapeHtml(source)}"></audio>
        `;
}

function activeAudioTrackForSelection(audioTracks: MediaAudioTrack[], selectedAudioStreamIndex: number | undefined): MediaAudioTrack | undefined {
  return audioTracks.find((track) => track.index === selectedAudioStreamIndex)
    ?? audioTracks.find((track) => track.default)
    ?? audioTracks[0];
}

function renderAudioTrackOptions(audioTracks: MediaAudioTrack[], activeAudioTrack: MediaAudioTrack | undefined): string {
  return audioTracks.map((track) => {
    const isActiveTrack = track.index === activeAudioTrack?.index;
    const activeTrackClass = isActiveTrack ? 'active' : '';
    const activeTrackChecked = isActiveTrack ? 'true' : 'false';
    const trackDetail = [track.language?.toUpperCase(), track.codec?.toUpperCase()].filter(Boolean).join(' · ')
      || (track.default ? 'Default' : 'Audio');
    return `
        <button class="player-track-option ${activeTrackClass}" type="button" role="menuitemradio" aria-checked="${activeTrackChecked}" data-player-audio-track-index="${track.index}">
          <span>${escapeHtml(track.label)}</span>
          <small>${escapeHtml(trackDetail)}</small>
        </button>
      `;
  }).join('');
}

function renderAudioTrackMenuMarkup(isAudio: boolean, audioTracks: MediaAudioTrack[], activeAudioTrack: MediaAudioTrack | undefined): string {
  if (isAudio || audioTracks.length <= 1) {
    return '';
  }

  const audioTrackMenuTitle = activeAudioTrack
    ? `Audio track: ${activeAudioTrack.label}`
    : 'Audio track changes may require remuxing';
  const audioTrackMenuExpanded = state.isAudioTrackMenuOpen ? 'true' : 'false';
  const audioTrackMenuClass = state.isAudioTrackMenuOpen ? '' : 'is-hidden';
  const audioTrackMenuHidden = state.isAudioTrackMenuOpen ? '' : 'hidden';
  return `
      <div class="player-menu-shell">
        <button id="player-audio-track-toggle" class="player-icon-button" type="button" title="${escapeHtml(audioTrackMenuTitle)}" aria-label="Audio track" aria-expanded="${audioTrackMenuExpanded}" aria-haspopup="menu">${renderIcon('languages', 'player-control-icon')}</button>
        <div id="player-audio-track-menu" class="player-track-menu ${audioTrackMenuClass}" role="menu" aria-label="Audio tracks" ${audioTrackMenuHidden}>
          ${renderAudioTrackOptions(audioTracks, activeAudioTrack)}
        </div>
      </div>
    `;
}

function renderPlayerTitleMarkup(logoUrl: string | undefined, title: string): string {
  return logoUrl
    ? `<img class="player-title-logo" src="${escapeHtml(logoUrl)}" alt="${escapeHtml(title)}" />`
    : `<h2>${escapeHtml(title)}</h2>`;
}

function renderTranscodeBadge(isRemuxingForAudio: boolean): string {
  const session = state.activePlaybackSession;
  if (!session) {
    return '';
  }

  const transcodeReason = isRemuxingForAudio
    ? 'Using this audio track requires a converted stream.'
    : session.decision.reason;
  const label = playbackDeliveryLabel(session.decision, isRemuxingForAudio);
  const className = label === 'Direct Play' ? 'is-direct' : 'is-transcoding';
  return `<span class="player-badge ${className}" title="${escapeHtml(transcodeReason)}">${label}</span>`;
}

function selectedAudioStreamIndexForPlayback(session: PlaybackSession): number | undefined {
  return state.activeAudioStreamIndex ?? session.audio_stream_index;
}

function playbackPosterUrl(playbackItem: MediaItemDetail): string | undefined {
  return playbackItem.poster_url
    ? getArtworkUrl(playbackItem.id, 'poster', playbackItem.artwork_updated_at)
    : undefined;
}

function playbackBackdropUrl(playbackItem: MediaItemDetail, posterUrl: string | undefined): string | undefined {
  return playbackItem.backdrop_url
    ? getArtworkUrl(playbackItem.id, 'backdrop', playbackItem.artwork_updated_at)
    : posterUrl;
}

function mediaStreamStartMs(session: PlaybackSession, isRemuxingForAudio: boolean): number {
  return session.decision.transcode_required || isRemuxingForAudio
    ? state.activePlaybackStartMs
    : 0;
}

function mediaPlayerShellClass(isAudio: boolean): string {
  return isAudio ? 'audio-player-shell' : 'video-player-shell';
}

function playerBackdropStyle(backdropUrl: string | undefined): string {
  return backdropUrl ? `style="--player-backdrop-image: url('${escapeHtml(backdropUrl)}');"` : '';
}

function renderPictureInPictureButton(isAudio: boolean): string {
  return isAudio
    ? ''
    : `<button id="player-pip" class="player-icon-button" type="button" title="Picture in picture" aria-label="Picture in picture">${renderIcon('picture-in-picture', 'player-control-icon')}</button>`;
}

function renderSubtitleSelection(item: MediaItemDetail): string {
  if (!item.subtitle_tracks.length) return '';
  const selected = state.playbackSettings?.subtitle;
  const options = item.subtitle_tracks.map((track, index) =>
    `<option value="${index}" ${track.label === selected ? 'selected' : ''}>${escapeHtml(track.label)}</option>`).join('');
  return `<label class="player-quality">Subtitles <select id="player-subtitles" aria-label="Subtitles"><option value="-1">Off</option>${options}</select></label>`;
}

function renderMediaPlayerOverlay(): string {
  const playbackItem = state.activePlaybackItem ?? state.selectedItem;
  const playbackSession = state.activePlaybackSession;
  if (!state.isPlayerOpen || !playbackItem) {
    return '';
  }

  const isAudio = playbackItem.media_kind === 'audio';
  const selectedAudioStreamIndex = playbackSession ? selectedAudioStreamIndexForPlayback(playbackSession) : state.activeAudioStreamIndex;
  const posterUrl = playbackPosterUrl(playbackItem);
  const backdropUrl = playbackBackdropUrl(playbackItem, posterUrl);
  const logoUrl = playbackItem.logo_url ? resolveApiUrl(playbackItem.logo_url) : undefined;
  const isAudioStreamOverride = selectedAudioStreamIndex !== undefined && selectedAudioStreamIndex > 0;
  const isRemuxingForAudio = isAudioStreamOverride && !playbackSession?.decision.transcode_required;
  const source = playbackSession ? getSessionStreamUrl(playbackSession.session_id, mediaStreamStartMs(playbackSession, isRemuxingForAudio), selectedAudioStreamIndex) : '';
  const audioTracks = playbackItem.audio_tracks ?? [];
  const activeAudioTrack = activeAudioTrackForSelection(audioTracks, selectedAudioStreamIndex);
  const mediaMarkup = source ? renderMediaElementMarkup(isAudio, source, posterUrl, renderSubtitleTrackMarkup(playbackItem, isAudio)) : '';
  const mediaElementMarkup = state.playbackSettings?.paused ? mediaMarkup.replace(' autoplay', '') : mediaMarkup;
  const audioTrackMenuMarkup = renderAudioTrackMenuMarkup(isAudio, audioTracks, activeAudioTrack);

  return `
    <div class="player-overlay media-player-overlay">
      <div class="player-shell media-player-shell ${mediaPlayerShellClass(isAudio)} is-controls-visible ${state.playbackProblem ? 'has-media-error' : 'is-media-loading'}" tabindex="-1" ${playerBackdropStyle(backdropUrl)}>
        ${mediaElementMarkup}
        <div class="player-loading-indicator" aria-live="polite">
          <span class="loading-spinner player-loading-spinner" aria-hidden="true"></span>
        </div>
        <div class="player-error-indicator" aria-live="polite">
          ${renderPlaybackProblem(state.playbackProblem)}
        </div>
        <div class="player-idle-hit-area" aria-hidden="true"></div>
        <div class="player-top-controls player-controls">
          <div class="player-title-block">
            <span class="eyebrow">Now playing</span>
            ${renderPlayerTitleMarkup(logoUrl, playbackItem.display_title)}
          </div>
          <div class="player-top-actions">
            ${renderTranscodeBadge(isRemuxingForAudio)}
            <button id="close-player" class="player-icon-button" type="button" title="Close" aria-label="Close player">${renderIcon('x', 'player-control-icon')}</button>
          </div>
        </div>
        <div class="player-bottom-controls player-controls">
          <input id="player-progress" class="player-progress" type="range" min="0" max="1000" value="0" step="1" aria-label="Playback position" />
          <div class="player-control-row">
            <div class="player-control-cluster player-time-cluster">
              <span class="player-time"><span id="player-current-time">0:00</span><span>/</span><span id="player-duration">${escapeHtml(formatDuration(playbackItem.duration_ms))}</span></span>
            </div>
            <div class="player-control-cluster player-transport-cluster">
              <button class="player-icon-button" type="button" data-player-seek="-10" title="Back 10 seconds" aria-label="Back 10 seconds">${renderIcon('skip-back', 'player-control-icon')}</button>
              <button id="player-play-toggle-small" class="player-icon-button player-primary-button" type="button" title="Pause" aria-label="Pause">${renderIcon('pause', 'player-control-icon')}</button>
              <button class="player-icon-button" type="button" data-player-seek="10" title="Forward 10 seconds" aria-label="Forward 10 seconds">${renderIcon('skip-forward', 'player-control-icon')}</button>
            </div>
            <div class="player-control-cluster player-tool-cluster">
              <button id="player-mute-toggle" class="player-icon-button" type="button" title="Mute" aria-label="Mute">${renderIcon('volume-2', 'player-control-icon')}</button>
              <input id="player-volume" class="player-volume" type="range" min="0" max="1" value="1" step="0.01" aria-label="Volume" />
              ${audioTrackMenuMarkup}
              ${isAudio ? '' : renderPlaybackQuality(state.activePlaybackQuality ?? 'original')}
              ${isAudio ? '' : renderSubtitleSelection(playbackItem)}
              ${renderPictureInPictureButton(isAudio)}
              <button id="player-fullscreen" class="player-icon-button" type="button" title="Fullscreen" aria-label="Fullscreen">${renderIcon('maximize', 'player-control-icon')}</button>
            </div>
          </div>
        </div>
      </div>
    </div>
  `;
}

function themeSongLayer(): HTMLElement {
  let layer = document.querySelector<HTMLElement>('#theme-song-layer');
  if (!layer) {
    layer = document.createElement('div');
    layer.id = 'theme-song-layer';
    document.body.appendChild(layer);
  }

  return layer;
}

function currentThemeSongSource(): ThemeSongSource | undefined {
  if (state.isPlayerOpen || state.activeTrailer || musicIsActive()) {
    return undefined;
  }

  const target = currentThemeSongTarget();
  return target ? themeSongSourceFromUrl(target.url, target.title) : undefined;
}

/** Opens a video overlay for an arbitrary playable trailer or extra option. */
export function openVideoOverlay(option: TrailerOption | undefined): void {
  if (!option) {
    return;
  }

  destroyTrailerYouTubePlayer();
  state.activeTrailer = option;
  state.isTrailerMenuOpen = false;
  render();
}

/** Opens the selected trailer option in the trailer overlay. */
export function openTrailer(option: TrailerOption | undefined): void {
  openVideoOverlay(option);
}

/** Closes the trailer overlay and tears down trailer playback resources. */
export function closeTrailerPlayer(): void {
  state.activeTrailer = undefined;
  destroyTrailerYouTubePlayer();
  render();
}

function themeSongSourceFromUrl(
  themeSongUrl: string,
  title: string,
): ThemeSongSource | undefined {
  if (!themeSongUrl) {
    return undefined;
  }

  const videoId = extractYouTubeVideoId(themeSongUrl);
  if (videoId) {
    return {
      kind: 'youtube',
      src: videoId,
      title,
      videoId,
    };
  }

  return {
    kind: 'audio',
    src: resolveApiUrl(themeSongUrl),
    title,
  };
}

function clearTrailerProgressHandle(): void {
  if (trailerProgressHandle !== undefined) {
    globalThis.clearInterval(trailerProgressHandle);
    trailerProgressHandle = undefined;
  }
}

function destroyTrailerYouTubePlayer(): void {
  clearTrailerProgressHandle();
  trailerYouTubePlayerReady = undefined;
  if (!trailerYouTubePlayer) {
    activeTrailerYouTubeVideoId = undefined;
    document.body.style.cursor = '';
    return;
  }

  try {
    trailerYouTubePlayer.pauseVideo();
    trailerYouTubePlayer.destroy();
  } catch {
    // The trailer iframe may already have been removed during a render.
  } finally {
    trailerYouTubePlayer = undefined;
    activeTrailerYouTubeVideoId = undefined;
    document.body.style.cursor = '';
  }
}

function destroyThemeSongYouTubePlayer(): void {
  themeSongYouTubePlayerReady = undefined;
  if (!themeSongYouTubePlayer) {
    return;
  }

  try {
    themeSongYouTubePlayer.pauseVideo();
    themeSongYouTubePlayer.destroy();
  } catch {
    // The YouTube iframe may already have been removed during a render.
  } finally {
    themeSongYouTubePlayer = undefined;
    activeThemeSongYouTubeVideoId = undefined;
  }
}

function ensureThemeSongYouTubePlayer(): Promise<YouTubePlayer> {
  if (themeSongYouTubePlayer) {
    return Promise.resolve(themeSongYouTubePlayer);
  }

  if (themeSongYouTubePlayerReady) {
    return themeSongYouTubePlayerReady;
  }

  const layer = themeSongLayer();
  if (!document.querySelector('#theme-song-youtube-player')) {
    layer.innerHTML = '<div id="theme-song-youtube-player" class="theme-song-iframe"></div>';
  }

  themeSongYouTubePlayerReady = loadYouTubeIframeApi().then((api) => new Promise<YouTubePlayer>((resolve) => {
    themeSongYouTubePlayer = new api.Player('theme-song-youtube-player', {
      height: '0',
      width: '0',
      videoId: YOUTUBE_THEME_PLACEHOLDER_VIDEO_ID,
      playerVars: {
        autoplay: 0,
        controls: 2,
        loop: 0,
      },
      events: {
        onReady: (event) => {
          event.target.setPlaybackQuality('small');
          resolve(event.target);
        },
        onStateChange: () => {
          if (state.hasDeferredAutoRefreshRender) {
            state.hasDeferredAutoRefreshRender = false;
            render();
          }
        },
        onError: (event) => {
          console.warn('YouTube theme song playback failed', {
            videoId: activeThemeSongYouTubeVideoId,
            errorCode: event.data,
          });
        },
      },
    });
  }));

  return themeSongYouTubePlayerReady;
}

/** Starts playback for the selected YouTube theme-song video. */
export function playYouTubeThemeSong(videoId: string): void {
  activeThemeSongYouTubeVideoId = videoId;
  if (themeSongYouTubePlayer) {
    themeSongYouTubePlayer.loadVideoById(videoId);
    return;
  }

  void ensureThemeSongYouTubePlayer().then((player) => {
    player.loadVideoById(videoId);
  });
}

/** Synchronizes the theme-song player with the currently selected item. */
export function syncThemeSongPlayer(): void {
  const layer = themeSongLayer();
  const source = currentThemeSongSource();
  if (!source) {
    destroyThemeSongYouTubePlayer();
    layer.replaceChildren();
    delete layer.dataset.themeKind;
    delete layer.dataset.themeSrc;
    return;
  }

  if (layer.hasChildNodes() && layer.dataset.themeKind === source.kind && layer.dataset.themeSrc === source.src) {
    return;
  }

  layer.dataset.themeKind = source.kind;
  layer.dataset.themeSrc = source.src;
  if (source.kind === 'youtube') {
    if (!document.querySelector('#theme-song-youtube-player')) {
      layer.innerHTML = '<div id="theme-song-youtube-player" class="theme-song-iframe"></div>';
    }
    playYouTubeThemeSong(source.videoId);
    return;
  }

  destroyThemeSongYouTubePlayer();
  layer.innerHTML = `<audio id="theme-song-player" autoplay preload="auto" src="${escapeHtml(source.src)}"></audio>`;
  const themePlayer = layer.querySelector<HTMLAudioElement>('#theme-song-player');
  if (!themePlayer) {
    return;
  }

  themePlayer.volume = 0.45;
  themePlayer.loop = false;
  themePlayer.addEventListener('ended', () => {
    if (state.hasDeferredAutoRefreshRender) {
      state.hasDeferredAutoRefreshRender = false;
      render();
    }
  }, { once: true });
  void themePlayer.play().catch(() => {
    // Autoplay can be blocked by the browser, so the page quietly falls back without looping.
  });
}

/** Stops any active browser playback session and clears playback state. */
export function closeActivePlaybackSession(): void {
  saveActivePlaybackProgress();
  state.isPlayerOpen = false;
  document.body.style.cursor = '';
  playbackRequest += 1;
  const sessionToClose = state.activePlaybackSession;
  state.activePlaybackItem = undefined;
  state.activePlaybackSession = undefined;
  state.activePlaybackStartMs = 0;
  state.activeAudioStreamIndex = undefined;
  state.isAudioTrackMenuOpen = false;
  state.playbackProblem = undefined;
  state.playbackSettings = undefined;
  render();
  if (sessionToClose) {
    deletePlaybackSession(sessionToClose.session_id)
      .catch((error) => {
        console.error('Failed to close playback session', error);
      })
      .finally(() => {
        void refreshData(false);
      });
  } else {
    void refreshData(false);
  }
}

/** Starts a browser playback session for an already-loaded media item. */
async function startMediaPlayback(item: MediaItemDetail, startMs: number): Promise<void> {
  saveActivePlaybackProgress();
  state.activePlaybackQuality = 'original';
  state.activePlaybackCompatible = false;
  state.playbackSettings = undefined;
  state.activeAudioStreamIndex = undefined;
  await replaceMediaPlayback(item, startMs);
}

async function replaceMediaPlayback(item: MediaItemDetail, startMs: number): Promise<void> {
  const request = ++playbackRequest;
  const account = state.bootstrap?.current_user?.id;
  state.activePlaybackCompatible ||= compatiblePlaybackSources.has(playbackSourceKey(item.id));
  const previousSession = state.activePlaybackSession;
  state.activePlaybackSession = undefined;
  state.activePlaybackItem = item;
  state.activePlaybackStartMs = Math.max(0, startMs);
  state.isPlayerOpen = true;
  state.playbackProblem = undefined;
  state.isAudioTrackMenuOpen = false;
  render();

  if (previousSession) {
    deletePlaybackSession(previousSession.session_id).catch((error) => {
      console.error('Failed to replace playback session', error);
    });
  }

  try {
    const session = await createPlaybackSession({
      item_id: item.id,
      client_profile: playbackProfile(getWebClientProfile(), state.activePlaybackQuality ?? 'original', state.activePlaybackCompatible ?? false),
    });
    if (request !== playbackRequest || account !== state.bootstrap?.current_user?.id) {
      await deletePlaybackSession(session.session_id);
      return;
    }
    state.activePlaybackSession = session;
  } catch (error) {
    if (request !== playbackRequest || account !== state.bootstrap?.current_user?.id) return;
    state.playbackProblem = playbackRequestProblem(error);
  }
  render();
}

function activePlaybackPositionMs(player = document.querySelector<HTMLMediaElement>('#media-player')): number {
  if (!player) return state.activePlaybackStartMs;
  const offset = Number(player.dataset.playbackOffsetMs ?? 0);
  const position = offset + Math.max(0, player.currentTime || 0) * 1000;
  // Keep a requested resume target if the stream failed before loading metadata.
  return Math.floor(player.readyState === 0 ? Math.max(position, state.activePlaybackStartMs) : position);
}

function activeProgressRequest(): { position_ms: number; duration_ms?: number; completed: boolean } {
  const player = document.querySelector<HTMLMediaElement>('#media-player');
  return { position_ms: activePlaybackPositionMs(player), duration_ms: state.activePlaybackItem?.duration_ms,
    completed: Boolean(player?.ended && !state.playbackProblem) };
}

function saveActivePlaybackProgress(): void {
  if (!state.activePlaybackSession || !state.activePlaybackItem) return;
  const progress = activeProgressRequest();
  document.querySelector<HTMLMediaElement>('#media-player')?.pause();
  void updatePlaybackProgress(state.activePlaybackItem.id, progress).catch((error) => console.warn('Failed to save playback position', error));
}

function finishActivePlaybackOnExit(): void {
  if (state.activePlaybackSession && state.activePlaybackItem) {
    finishPlaybackOnExit(state.activePlaybackSession.session_id, state.activePlaybackItem.id, activeProgressRequest());
  }
}

/** Recreates a session at the source position; automatic recovery can preserve the intended pause state. */
export async function restartMediaPlayback(quality?: PlaybackQuality, compatible?: boolean, paused?: boolean): Promise<void> {
  const item = state.activePlaybackItem;
  if (!state.isPlayerOpen || !item) return;
  const player = document.querySelector<HTMLMediaElement>('#media-player');
  const position = activePlaybackPositionMs(player);
  if (player) {
    rememberPlaybackSettings(player);
  }
  if (paused !== undefined && state.playbackSettings) state.playbackSettings.paused = paused;
  saveActivePlaybackProgress();
  if (quality) {
    state.activePlaybackQuality = quality;
    state.activePlaybackCompatible = false;
  }
  if (compatible !== undefined) state.activePlaybackCompatible = compatible;
  await replaceMediaPlayback(item, position);
}

function playbackSourceKey(itemId: number): string {
  return `${state.apiBase}\n${state.bootstrap?.current_user?.id}\n${itemId}`;
}

function rememberPlaybackSettings(player: HTMLMediaElement): void {
  state.playbackSettings = { paused: player.paused && !state.playbackProblem, volume: player.volume, muted: player.muted,
    subtitle: Array.from(player.textTracks).find((track) => track.mode === 'showing')?.label };
}

function showPlaybackProblem(problem: PlaybackProblem): void {
  state.playbackProblem = problem;
  const shell = document.querySelector<HTMLElement>('.media-player-shell');
  shell?.classList.remove('is-media-loading', 'is-controls-hidden');
  shell?.classList.add('has-media-error', 'is-controls-visible');
  const indicator = shell?.querySelector<HTMLElement>('.player-error-indicator');
  if (indicator) indicator.innerHTML = renderPlaybackProblem(problem);
}

async function reportMediaError(player: HTMLMediaElement, code = player.error?.code ?? 0): Promise<void> {
  const session = state.activePlaybackSession;
  const request = playbackRequest;
  const source = player.getAttribute('src');
  const isCurrent = (): boolean => request === playbackRequest && state.isPlayerOpen
    && player.isConnected && document.querySelector('#media-player') === player && player.getAttribute('src') === source;
  if (!session || !isCurrent() || reportedPlaybackErrors.get(player) === source) return;
  reportedPlaybackErrors.set(player, source);
  const copiedVideo = !session.decision.video_transcode_required
    && (state.activeAudioStreamIndex ?? session.audio_stream_index ?? 0) === 0;
  const canRecover = (code === 3 || code === 4) && copiedVideo && !state.activePlaybackCompatible;
  if (!canRecover) showPlaybackProblem(mediaPlaybackProblem(code, state.activePlaybackCompatible ?? false));
  try {
    const status = await getPlaybackSessionStatus(session.session_id);
    if (!isCurrent()) return;
    if (canRecover && !status.error) {
      const paused = state.playbackSettings?.paused ?? false;
      // Capability queries can succeed even when the browser cannot create the actual decoder.
      state.playbackProblem = mediaPlaybackProblem(code, false);
      await restartMediaPlayback(undefined, true, paused);
      return;
    }
    showPlaybackProblem(mediaPlaybackProblem(code, state.activePlaybackCompatible ?? false, status));
  } catch (error) {
    if (isCurrent()) showPlaybackProblem(playbackRequestProblem(error));
  }
}

function bindPlaybackRecovery(): void {
  document.querySelector('.media-player-shell .player-error-indicator')?.addEventListener('click', (event) => {
    const target = (event.target as Element).closest('button');
    if (target?.hasAttribute('data-player-retry')) void restartMediaPlayback();
    if (target?.hasAttribute('data-player-compatible')) void restartMediaPlayback(undefined, true);
    if (target?.hasAttribute('data-player-sign-in')) globalThis.location.reload();
  });
  document.querySelector<HTMLSelectElement>('#player-quality')?.addEventListener('change', (event) => {
    const quality = (event.target as HTMLSelectElement).value;
    const selected = PLAYBACK_QUALITIES.find((entry) => entry.id === quality);
    if (selected) void restartMediaPlayback(selected.id);
  });
  document.querySelector<HTMLSelectElement>('#player-subtitles')?.addEventListener('change', (event) => {
    const selected = Number((event.target as HTMLSelectElement).value);
    const player = document.querySelector<HTMLMediaElement>('#media-player');
    if (!player) return;
    for (const [index, track] of Array.from(player.textTracks).entries()) track.mode = index === selected ? 'showing' : 'disabled';
    rememberPlaybackSettings(player);
  });
}

/** Start an album in disc/track order, optionally beginning at a selected track. */
export async function startMusicPlayback(item: MediaItemDetail, startMs = 0): Promise<void> {
  if (state.isPlayerOpen) closeActivePlaybackSession();
  await playMusicItem(item, startMs);
}

/** Start a track with its remaining album queue, or another playable media item. */
export async function startPlayback(item: MediaItemDetail, startMs: number): Promise<void> {
  if (item.item_type === 'podcast_episode' && item.media_kind === 'audio') {
    if (state.isPlayerOpen) closeActivePlaybackSession();
    await playPodcastItem(item, startMs);
    return;
  }
  if (['audiobook', 'audiobook_part'].includes(item.item_type)) {
    if (state.isPlayerOpen) closeActivePlaybackSession();
    await playAudiobookItem(item, startMs);
    return;
  }
  if (item.item_type === 'track' || item.item_type === 'album') {
    await startMusicPlayback(item, startMs);
    return;
  }
  stopMusic();
  await startMediaPlayback(item, startMs);
}

/** Load an item and play it; omitting an audiobook offset resumes the account's saved file and position. */
export async function startPlaybackForItemId(itemId: number, startMs?: number): Promise<void> {
  const request = ++playbackRequest;
  const account = state.bootstrap?.current_user?.id;
  const item = state.selectedItem?.id === itemId
    ? state.selectedItem
    : await getItem(itemId);
  if (request !== playbackRequest || account !== state.bootstrap?.current_user?.id) return;
  if (item.item_type === 'audiobook' && startMs === undefined && item.playback_target) {
    const target = await getItem(item.playback_target.item_id);
    if (request !== playbackRequest || account !== state.bootstrap?.current_user?.id) return;
    await startPlayback(target, item.playback_target.start_ms);
  } else await startPlayback(item, startMs ?? 0);
}

/** Start the current file or the entire narration over using the same queue and history rules as music. */
export async function restartAudiobook(itemId: number, scope: AudiobookRestartScope): Promise<void> {
  const request = ++playbackRequest, account = state.bootstrap?.current_user?.id;
  const book = await getItem(itemId);
  if (request !== playbackRequest || account !== state.bootstrap?.current_user?.id) return;
  if (book.item_type !== 'audiobook') throw new Error('This audiobook is unavailable.');
  const target = audiobookRestartTarget(book, scope);
  if (!target) throw new Error('No audio files are available for this audiobook.');
  await startPlaybackForItemId(target.id, 0);
}

function ensureTrailerYouTubePlayer(videoId: string): Promise<YouTubePlayer> {
  if (trailerYouTubePlayer && activeTrailerYouTubeVideoId === videoId) {
    return Promise.resolve(trailerYouTubePlayer);
  }

  if (trailerYouTubePlayerReady && activeTrailerYouTubeVideoId === videoId) {
    return trailerYouTubePlayerReady;
  }

  destroyTrailerYouTubePlayer();
  activeTrailerYouTubeVideoId = videoId;
  const playerVars: Record<string, number | string> = {
    autoplay: 1,
    controls: 0,
    disablekb: 1,
    fs: 0,
    iv_load_policy: 3,
    loop: 0,
    modestbranding: 1,
    playsinline: 1,
    rel: 0,
  };
  if (globalThis.location.origin.startsWith('http')) {
    playerVars.origin = globalThis.location.origin;
  }

  trailerYouTubePlayerReady = loadYouTubeIframeApi().then((api) => new Promise<YouTubePlayer>((resolve) => {
    trailerYouTubePlayer = new api.Player('trailer-player', {
      height: '100%',
      width: '100%',
      videoId,
      playerVars,
      events: {
        onReady: (event) => {
          trailerYouTubePlayer = event.target;
          event.target.setPlaybackQuality('hd720');
          event.target.setVolume(Math.round(trailerVolume * 100));
          if (trailerMuted) {
            event.target.mute();
          } else {
            event.target.unMute();
          }
          event.target.playVideo();
          resolve(event.target);
        },
        onStateChange: () => {
          updateTrailerPlayerUi();
        },
        onError: (event) => {
          document.querySelector<HTMLElement>('.trailer-shell')?.classList.add('has-media-error');
          document.querySelector<HTMLElement>('.trailer-shell')?.classList.remove('is-media-loading');
          console.warn('YouTube trailer playback failed', {
            videoId: activeTrailerYouTubeVideoId,
            errorCode: event.data,
          });
        },
      },
    });
  }));

  return trailerYouTubePlayerReady;
}

function trailerPlayerState(): number | undefined {
  try {
    return trailerYouTubePlayer?.getPlayerState();
  } catch {
    return undefined;
  }
}

function isTrailerPlaying(): boolean {
  return trailerPlayerState() === YOUTUBE_PLAYER_STATE.playing;
}

function updateIconButton(
  button: HTMLButtonElement | null | undefined,
  iconName: AppIconName,
  label: string,
): void {
  if (!button) {
    return;
  }
  button.innerHTML = renderIcon(iconName, 'player-control-icon');
  button.title = label;
  button.setAttribute('aria-label', label);
  createIcons({ icons });
}

/** Creates a seek handler that increases the step when repeated in one direction. */
function createEscalatingSeekHandler(seekBy: (seconds: number) => void): (direction: number) => void {
  let lastSkipDirection = 0;
  let lastSkipAt = 0;
  let skipStepIndex = 0;

  return (direction: number): void => {
    const now = Date.now();
    if (direction !== 0 && direction === lastSkipDirection && now - lastSkipAt < ESCALATING_SEEK_WINDOW_MS) {
      skipStepIndex = Math.min(ESCALATING_SEEK_STEPS.length - 1, skipStepIndex + 1);
    } else {
      skipStepIndex = 0;
    }
    lastSkipDirection = direction;
    lastSkipAt = now;
    seekBy(direction * ESCALATING_SEEK_STEPS[skipStepIndex]);
  };
}

function updateTrailerPlayerUi(): void {
  const player = trailerYouTubePlayer;
  if (!player) {
    return;
  }

  const shell = document.querySelector<HTMLElement>('.trailer-shell');
  const progress = document.querySelector<HTMLInputElement>('#trailer-progress');
  const currentTimeLabel = document.querySelector<HTMLElement>('#trailer-current-time');
  const durationLabel = document.querySelector<HTMLElement>('#trailer-duration');
  const playButtons = Array.from(document.querySelectorAll<HTMLButtonElement>('#trailer-play-toggle-small'));
  const muteButton = document.querySelector<HTMLButtonElement>('#trailer-mute-toggle');
  const volume = document.querySelector<HTMLInputElement>('#trailer-volume');
  const playerState = trailerPlayerState();
  const isPlaying = playerState === YOUTUBE_PLAYER_STATE.playing;
  const isLoading = playerState === YOUTUBE_PLAYER_STATE.buffering || playerState === YOUTUBE_PLAYER_STATE.cued;
  const duration = player.getDuration();
  const currentTime = player.getCurrentTime();

  shell?.classList.toggle('is-media-loading', isLoading);
  playButtons.forEach((button) => updateIconButton(button, isPlaying ? 'pause' : 'play', isPlaying ? 'Pause' : 'Play'));
  trailerMuted = player.isMuted() || player.getVolume() === 0;
  trailerVolume = Math.max(0, Math.min(1, player.getVolume() / 100));
  updateIconButton(muteButton, trailerMuted ? 'volume-x' : 'volume-2', trailerMuted ? 'Unmute' : 'Mute');
  if (volume && document.activeElement !== volume) {
    volume.value = String(trailerMuted ? 0 : trailerVolume);
  }
  if (progress && progress.dataset.scrubbing !== 'true') {
    progress.value = duration > 0 ? String(Math.min(1000, Math.max(0, (currentTime / duration) * 1000))) : '0';
  }
  if (currentTimeLabel) {
    currentTimeLabel.textContent = formatMediaTime(currentTime);
  }
  if (durationLabel) {
    durationLabel.textContent = formatMediaTime(duration);
  }
}

/** Binds controls for the trailer overlay player in the current DOM tree. */
export function bindTrailerPlayer(): void {
  if (!state.activeTrailer) {
    destroyTrailerYouTubePlayer();
    return;
  }

  const shell = document.querySelector<HTMLElement>('.trailer-shell');
  const videoId = shell?.dataset.trailerVideoId;
  if (!shell || !videoId) {
    return;
  }

  const progress = document.querySelector<HTMLInputElement>('#trailer-progress');
  const volume = document.querySelector<HTMLInputElement>('#trailer-volume');
  const currentTimeLabel = document.querySelector<HTMLElement>('#trailer-current-time');
  const playButtons = Array.from(document.querySelectorAll<HTMLButtonElement>('#trailer-play-toggle-small'));
  const muteButton = document.querySelector<HTMLButtonElement>('#trailer-mute-toggle');
  const fullscreenButton = document.querySelector<HTMLButtonElement>('#trailer-fullscreen');
  const idleHitArea = document.querySelector<HTMLElement>('.trailer-idle-hit-area');
  let controlsHideHandle: number | undefined;
  let isScrubbing = false;

  const withTrailerPlayer = (action: (player: YouTubePlayer) => void): void => {
    if (trailerYouTubePlayer) {
      action(trailerYouTubePlayer);
      updateTrailerPlayerUi();
      return;
    }
    void ensureTrailerYouTubePlayer(videoId).then((player) => {
      action(player);
      updateTrailerPlayerUi();
    });
  };

  const showControls = (): void => {
    shell.classList.add('is-controls-visible');
    shell.classList.remove('is-controls-hidden');
    document.body.style.cursor = '';
    if (controlsHideHandle !== undefined) {
      globalThis.clearTimeout(controlsHideHandle);
    }
    controlsHideHandle = globalThis.setTimeout(() => {
      if (isTrailerPlaying() && !isScrubbing) {
        shell.classList.remove('is-controls-visible');
        shell.classList.add('is-controls-hidden');
        document.body.style.cursor = 'none';
      }
    }, 3200);
  };

  const seekBy = (seconds: number): void => {
    withTrailerPlayer((player) => {
      const duration = player.getDuration();
      const currentTime = player.getCurrentTime();
      const targetTime = duration > 0
        ? Math.min(duration, Math.max(0, currentTime + seconds))
        : Math.max(0, currentTime + seconds);
      player.seekTo(targetTime, true);
    });
  };

  const seekWithEscalation = createEscalatingSeekHandler(seekBy);

  const togglePlayback = (): void => {
    withTrailerPlayer((player) => {
      if (player.getPlayerState() === YOUTUBE_PLAYER_STATE.playing) {
        player.pauseVideo();
      } else {
        player.playVideo();
      }
    });
  };

  const toggleFullscreen = (): void => {
    if (document.fullscreenElement) {
      void document.exitFullscreen();
      return;
    }
    void shell.requestFullscreen?.();
  };

  shell.focus({ preventScroll: true });
  ['mousemove', 'mousedown', 'touchstart', 'pointermove'].forEach((eventName) => {
    shell.addEventListener(eventName, showControls, { passive: true });
  });
  shell.addEventListener('keydown', (event) => {
    if (event.target instanceof Element && event.target.closest('input, select, button')) {
      return;
    }
    if (event.key === ' ' || event.key === 'k') {
      event.preventDefault();
      togglePlayback();
    } else if (event.key === 'ArrowLeft') {
      event.preventDefault();
      seekWithEscalation(-1);
    } else if (event.key === 'ArrowRight') {
      event.preventDefault();
      seekWithEscalation(1);
    } else if (event.key === 'm') {
      event.preventDefault();
      muteButton?.click();
    } else if (event.key === 'f') {
      event.preventDefault();
      toggleFullscreen();
    } else if (event.key === 'Escape') {
      event.preventDefault();
      closeTrailerPlayer();
    }
    showControls();
  });

  idleHitArea?.addEventListener('click', () => {
    togglePlayback();
    showControls();
  });
  playButtons.forEach((button) => button.addEventListener('click', () => {
    togglePlayback();
    showControls();
  }));
  document.querySelectorAll<HTMLButtonElement>('[data-trailer-seek]').forEach((button) => {
    button.addEventListener('click', () => {
      const requestedSeconds = Number(button.dataset.trailerSeek);
      const direction = Math.sign(requestedSeconds);
      if (direction !== 0) {
        seekWithEscalation(direction);
      }
      showControls();
    });
  });
  muteButton?.addEventListener('click', () => {
    withTrailerPlayer((player) => {
      if (player.isMuted() || player.getVolume() === 0) {
        player.unMute();
        if (player.getVolume() === 0) {
          player.setVolume(Math.round(Math.max(trailerVolume, 0.45) * 100));
        }
      } else {
        player.mute();
      }
    });
    showControls();
  });
  volume?.addEventListener('input', () => {
    const nextVolume = Math.min(1, Math.max(0, Number(volume.value)));
    trailerVolume = nextVolume;
    withTrailerPlayer((player) => {
      player.setVolume(Math.round(nextVolume * 100));
      if (nextVolume <= 0) {
        player.mute();
      } else {
        player.unMute();
      }
    });
    showControls();
  });
  volume?.addEventListener('wheel', (event) => {
    event.preventDefault();
    const delta = event.deltaY < 0 ? 0.05 : -0.05;
    const nextVolume = Math.min(1, Math.max(0, trailerVolume + delta));
    trailerVolume = nextVolume;
    withTrailerPlayer((player) => {
      player.setVolume(Math.round(nextVolume * 100));
      if (nextVolume <= 0) {
        player.mute();
      } else {
        player.unMute();
      }
    });
    showControls();
  }, { passive: false });
  fullscreenButton?.addEventListener('click', () => {
    toggleFullscreen();
    showControls();
  });
  progress?.addEventListener('input', () => {
    isScrubbing = true;
    progress.dataset.scrubbing = 'true';
    if (!trailerYouTubePlayer) {
      return;
    }
    const duration = trailerYouTubePlayer.getDuration();
    if (duration > 0 && currentTimeLabel) {
      currentTimeLabel.textContent = formatMediaTime((Number(progress.value) / 1000) * duration);
    }
    showControls();
  });
  progress?.addEventListener('wheel', (event) => {
    event.preventDefault();
    seekWithEscalation(event.deltaY < 0 ? 1 : -1);
    showControls();
  }, { passive: false });
  progress?.addEventListener('change', () => {
    if (trailerYouTubePlayer) {
      const duration = trailerYouTubePlayer.getDuration();
      if (duration > 0) {
        trailerYouTubePlayer.seekTo((Number(progress.value) / 1000) * duration, true);
      }
    }
    isScrubbing = false;
    delete progress.dataset.scrubbing;
    updateTrailerPlayerUi();
    showControls();
  });

  shell.classList.add('is-media-loading');
  void ensureTrailerYouTubePlayer(videoId).then((player) => {
    player.playVideo();
    updateTrailerPlayerUi();
    clearTrailerProgressHandle();
    trailerProgressHandle = globalThis.setInterval(updateTrailerPlayerUi, 500);
    showControls();
  });
}

/** Binds controls for the browser playback overlay in the current DOM tree. */
export function bindPlayerProgress(): void {
  bindPlaybackRecovery();
  const player = document.querySelector<HTMLMediaElement>('#media-player');
  const playbackItem = state.activePlaybackItem ?? state.selectedItem;
  if (!player || !playbackItem) {
    return;
  }

  const shell = document.querySelector<HTMLElement>('.media-player-shell');
  const progress = document.querySelector<HTMLInputElement>('#player-progress');
  const volume = document.querySelector<HTMLInputElement>('#player-volume');
  const currentTimeLabel = document.querySelector<HTMLElement>('#player-current-time');
  const durationLabel = document.querySelector<HTMLElement>('#player-duration');
  const playButtons = Array.from(document.querySelectorAll<HTMLButtonElement>('#player-play-toggle-small'));
  const muteButton = document.querySelector<HTMLButtonElement>('#player-mute-toggle');
  const fullscreenButton = document.querySelector<HTMLButtonElement>('#player-fullscreen');
  const pipButton = document.querySelector<HTMLButtonElement>('#player-pip');
  const audioTrackToggle = document.querySelector<HTMLButtonElement>('#player-audio-track-toggle');
  const audioTrackMenu = document.querySelector<HTMLElement>('#player-audio-track-menu');
  const selectedAudioStreamIndex = state.activeAudioStreamIndex ?? state.activePlaybackSession?.audio_stream_index;
  const currentAudioTrackIndex = selectedAudioStreamIndex ?? 0;
  const isAudioStreamOverride = selectedAudioStreamIndex !== undefined && selectedAudioStreamIndex > 0;
  const isTranscoding = (state.activePlaybackSession?.decision.transcode_required ?? false) || isAudioStreamOverride;
  const sourceDurationSeconds = (playbackItem.duration_ms ?? 0) / 1000;
  const requestedPlaybackStartSeconds = Math.max(0, state.activePlaybackStartMs / 1000);
  const playbackBaseOffsetSeconds = isTranscoding ? requestedPlaybackStartSeconds : 0;
  const initialDirectSeekSeconds = isTranscoding ? 0 : requestedPlaybackStartSeconds;
  player.dataset.playbackOffsetMs = String(playbackBaseOffsetSeconds * 1000);
  const source = player.getAttribute('src') ?? '';
  let initialization = initializedMediaSources.get(player);
  initialization?.cleanup?.();
  if (!initialization || initialization.source !== source) {
    initialization = { source, seekApplied: false, settingsApplied: false, started: false };
    initializedMediaSources.set(player, initialization);
  }
  const mediaInitialization = initialization;
  let controlsHideHandle: number | undefined;
  let isScrubbing = false;
  let hasAppliedInitialDirectSeek = initialDirectSeekSeconds <= 0 || mediaInitialization.seekApplied;
  let stallHandle: number | undefined;
  mediaInitialization.cleanup = () => {
    if (controlsHideHandle !== undefined) globalThis.clearTimeout(controlsHideHandle);
    if (stallHandle !== undefined) globalThis.clearTimeout(stallHandle);
  };

  const trackPlaybackStall = (loading: boolean): void => {
    if (!loading || player.paused || state.playbackProblem) {
      if (stallHandle !== undefined) globalThis.clearTimeout(stallHandle);
      stallHandle = undefined;
      return;
    }
    if (stallHandle !== undefined) return;
    const source = player.getAttribute('src');
    stallHandle = globalThis.setTimeout(() => {
      if (player.isConnected && player.getAttribute('src') === source && !player.paused
        && player.readyState < player.HAVE_FUTURE_DATA && !state.playbackProblem) {
        void reportMediaError(player, 2);
      }
    }, 20000);
  };

  const playbackDurationSeconds = (): number => {
    if (sourceDurationSeconds > 0) {
      return sourceDurationSeconds;
    }
    if (Number.isFinite(player.duration) && player.duration > 0) {
      return player.duration;
    }
    return 0;
  };

  const setPlayerLoading = (loading: boolean): void => {
    const shouldShowLoading = loading && !player.ended && player.readyState < player.HAVE_FUTURE_DATA;
    shell?.classList.toggle('is-media-loading', shouldShowLoading);
    trackPlaybackStall(shouldShowLoading);
    if (!state.playbackProblem) shell?.classList.remove('has-media-error');
  };

  const refreshPlayerLoading = (): void => {
    setPlayerLoading(!player.paused && player.readyState < player.HAVE_FUTURE_DATA);
  };

  const updatePlayButtons = (): void => {
    const iconName: AppIconName = player.paused ? 'play' : 'pause';
    const label = player.paused ? 'Play' : 'Pause';
    playButtons.forEach((button) => updateIconButton(button, iconName, label));
  };

  const updateMuteButton = (): void => {
    updateIconButton(muteButton, player.muted || player.volume === 0 ? 'volume-x' : 'volume-2', player.muted ? 'Unmute' : 'Mute');
    if (volume && !isScrubbing) {
      volume.value = String(player.muted ? 0 : player.volume);
    }
  };

  const updatePipButton = (): void => {
    if (!pipButton || !(player instanceof HTMLVideoElement)) {
      return;
    }
    const isSupported = document.pictureInPictureEnabled && !player.disablePictureInPicture;
    pipButton.disabled = !isSupported;
    pipButton.title = isSupported ? 'Picture in picture' : 'Picture in picture is not available in this browser';
    pipButton.setAttribute('aria-label', pipButton.title);
  };

  const setAudioTrackMenuOpen = (open: boolean): void => {
    state.isAudioTrackMenuOpen = open;
    audioTrackToggle?.setAttribute('aria-expanded', open ? 'true' : 'false');
    audioTrackMenu?.classList.toggle('is-hidden', !open);
    audioTrackMenu?.toggleAttribute('hidden', !open);
  };

  const updateTimeline = (): void => {
    const duration = playbackDurationSeconds();
    const currentPosition = Math.min(duration || Number.POSITIVE_INFINITY, playbackBaseOffsetSeconds + player.currentTime);
    if (progress && !isScrubbing) {
      progress.value = duration > 0 ? String(Math.min(1000, Math.max(0, (currentPosition / duration) * 1000))) : '0';
    }
    if (currentTimeLabel) {
      currentTimeLabel.textContent = formatMediaTime(currentPosition);
    }
    if (durationLabel) {
      durationLabel.textContent = formatMediaTime(duration);
    }
  };

  const applyInitialDirectSeek = (): void => {
    if (hasAppliedInitialDirectSeek || initialDirectSeekSeconds <= 0 || player.readyState < player.HAVE_METADATA) {
      return;
    }

    const duration = playbackDurationSeconds();
    const targetPosition = duration > 0
      ? Math.min(initialDirectSeekSeconds, Math.max(0, duration - 1))
      : initialDirectSeekSeconds;

    try {
      player.currentTime = targetPosition;
      hasAppliedInitialDirectSeek = true;
      mediaInitialization.seekApplied = true;
      updateTimeline();
    } catch (error) {
      console.warn('Failed to seek direct-play item to resume position', error);
    }
  };

  const showControls = (): void => {
    shell?.classList.add('is-controls-visible');
    shell?.classList.remove('is-controls-hidden');
    document.body.style.cursor = '';
    if (controlsHideHandle !== undefined) {
      globalThis.clearTimeout(controlsHideHandle);
    }
    controlsHideHandle = globalThis.setTimeout(() => {
      if (!player.paused && !isScrubbing && !state.playbackProblem && document.activeElement?.closest('.player-controls') === null) {
        shell?.classList.remove('is-controls-visible');
        shell?.classList.add('is-controls-hidden');
        document.body.style.cursor = 'none';
      }
    }, 3200);
  };

  const seekBy = (seconds: number): void => {
    const currentPosition = playbackBaseOffsetSeconds + player.currentTime;
    const limit = sourceDurationSeconds > 0 ? Math.max(0, sourceDurationSeconds - 0.25) : Number.POSITIVE_INFINITY;
    const targetPosition = Math.min(limit, Math.max(0, currentPosition + seconds));
    if (isTranscoding) {
      rememberPlaybackSettings(player);
      state.activePlaybackStartMs = Math.floor(targetPosition * 1000);
      render(false);
      return;
    }
    if (!Number.isFinite(player.duration)) {
      player.currentTime = targetPosition;
      return;
    }
    player.currentTime = Math.min(player.duration, targetPosition);
  };

  const seekWithEscalation = createEscalatingSeekHandler(seekBy);

  const togglePlayback = (): void => {
    rememberPlaybackSettings(player);
    state.playbackSettings!.paused = !player.paused;
    if (player.paused) {
      void player.play();
    } else {
      player.pause();
    }
  };

  const toggleFullscreen = (): void => {
    const fullscreenElement = document.fullscreenElement;
    if (fullscreenElement) {
      void document.exitFullscreen();
      return;
    }
    void shell?.requestFullscreen?.();
  };

  shell?.focus({ preventScroll: true });
  ['mousemove', 'mousedown', 'touchstart', 'pointermove'].forEach((eventName) => {
    shell?.addEventListener(eventName, showControls, { passive: true });
  });
  shell?.addEventListener('keydown', (event) => {
    if (event.target instanceof HTMLInputElement) {
      return;
    }
    if (event.key === ' ' || event.key === 'k') {
      event.preventDefault();
      togglePlayback();
    } else if (event.key === 'ArrowLeft') {
      event.preventDefault();
      seekBy(-10);
    } else if (event.key === 'ArrowRight') {
      event.preventDefault();
      seekBy(30);
    } else if (event.key === 'm') {
      event.preventDefault();
      player.muted = !player.muted;
      updateMuteButton();
    } else if (event.key === 'f') {
      event.preventDefault();
      toggleFullscreen();
    } else if (event.key === 'Escape') {
      event.preventDefault();
      closeActivePlaybackSession();
    }
    showControls();
  });

  playButtons.forEach((button) => button.addEventListener('click', () => {
    togglePlayback();
    showControls();
  }));
  document.querySelectorAll<HTMLButtonElement>('[data-player-seek]').forEach((button) => {
    button.addEventListener('click', () => {
      const requestedSeconds = Number(button.dataset.playerSeek);
      const direction = Math.sign(requestedSeconds);
      if (direction !== 0) {
        seekWithEscalation(direction);
      }
      showControls();
    });
  });
  muteButton?.addEventListener('click', () => {
    player.muted = !player.muted;
    updateMuteButton();
    showControls();
  });
  volume?.addEventListener('input', () => {
    player.volume = Number(volume.value);
    player.muted = player.volume === 0;
    updateMuteButton();
    showControls();
  });
  volume?.addEventListener('wheel', (event) => {
    event.preventDefault();
    const delta = event.deltaY < 0 ? 0.05 : -0.05;
    player.volume = Math.min(1, Math.max(0, player.volume + delta));
    player.muted = player.volume === 0;
    updateMuteButton();
    showControls();
  }, { passive: false });
  fullscreenButton?.addEventListener('click', () => {
    toggleFullscreen();
    showControls();
  });
  audioTrackToggle?.addEventListener('click', () => {
    setAudioTrackMenuOpen(!state.isAudioTrackMenuOpen);
    showControls();
  });
  document.querySelectorAll<HTMLButtonElement>('[data-player-audio-track-index]').forEach((button) => {
    button.addEventListener('click', () => {
      const nextAudioTrackIndex = Number(button.dataset.playerAudioTrackIndex);
      if (!Number.isFinite(nextAudioTrackIndex)) {
        return;
      }
      if (nextAudioTrackIndex === currentAudioTrackIndex) {
        setAudioTrackMenuOpen(false);
        showControls();
        return;
      }
      state.activeAudioStreamIndex = nextAudioTrackIndex;
      rememberPlaybackSettings(player);
      state.activePlaybackStartMs = Math.floor((playbackBaseOffsetSeconds + player.currentTime) * 1000);
      setAudioTrackMenuOpen(false);
      render(false);
    });
  });
  pipButton?.addEventListener('click', async () => {
    if (!(player instanceof HTMLVideoElement) || !document.pictureInPictureEnabled) {
      state.error = 'Picture in picture is not available in this browser.';
      render();
      return;
    }
    try {
      if (document.fullscreenElement) {
        await document.exitFullscreen();
      }
      if (player.paused) {
        void player.play();
      }
      await player.requestPictureInPicture();
      shell?.classList.add('is-picture-in-picture');
      document.body.style.cursor = '';
    } catch (error) {
      console.error('Failed to enter picture-in-picture', error);
      state.error = error instanceof Error ? error.message : 'Failed to enter picture in picture.';
      render();
    }
  });
  player.addEventListener('leavepictureinpicture', () => {
    shell?.classList.remove('is-picture-in-picture');
    showControls();
  });
  progress?.addEventListener('input', () => {
    isScrubbing = true;
    const duration = playbackDurationSeconds();
    if (duration > 0) {
      const previewSeconds = (Number(progress.value) / 1000) * duration;
      if (currentTimeLabel) {
        currentTimeLabel.textContent = formatMediaTime(previewSeconds);
      }
    }
    showControls();
  });
  progress?.addEventListener('wheel', (event) => {
    event.preventDefault();
    const direction = event.deltaY < 0 ? 1 : -1;
    seekWithEscalation(direction);
    updateTimeline();
    showControls();
  }, { passive: false });
  progress?.addEventListener('change', () => {
    const duration = playbackDurationSeconds();
    if (duration > 0) {
      const targetPosition = Math.min(Math.max(0, duration - 0.25), (Number(progress.value) / 1000) * duration);
      if (isTranscoding) {
        rememberPlaybackSettings(player);
        state.activePlaybackStartMs = Math.floor(targetPosition * 1000);
        render(false);
        return;
      }
      player.currentTime = targetPosition;
    }
    isScrubbing = false;
    updateTimeline();
    showControls();
  });

  let lastSentSeconds = -1;
  player.addEventListener('loadstart', () => setPlayerLoading(true));
  player.addEventListener('waiting', refreshPlayerLoading);
  player.addEventListener('stalled', refreshPlayerLoading);
  player.addEventListener('loadeddata', () => setPlayerLoading(false));
  player.addEventListener('canplay', () => {
    applyInitialDirectSeek();
    setPlayerLoading(false);
  });
  player.addEventListener('playing', () => {
    setPlayerLoading(false);
    if (state.activePlaybackCompatible && state.activePlaybackItem?.id === playbackItem.id) {
      compatiblePlaybackSources.add(playbackSourceKey(playbackItem.id));
    }
  });
  player.addEventListener('error', () => {
    void reportMediaError(player);
    console.error('Media playback failed', player.error);
  });
  player.addEventListener('loadedmetadata', () => {
    applyInitialDirectSeek();
    applyPlaybackSettings();
    updateTimeline();
  });
  player.addEventListener('play', () => {
    updatePlayButtons();
    refreshPlayerLoading();
    showControls();
  });
  player.addEventListener('pause', () => {
    updatePlayButtons();
    refreshPlayerLoading();
    showControls();
  });
  player.addEventListener('volumechange', updateMuteButton);
  player.addEventListener('timeupdate', () => {
    refreshPlayerLoading();
    updateTimeline();
    const currentSeconds = Math.floor(player.currentTime);
    if (lastSentSeconds >= 0 && Math.abs(currentSeconds - lastSentSeconds) < 15) {
      return;
    }

    lastSentSeconds = currentSeconds;
    void updatePlaybackProgress(playbackItem.id, {
      position_ms: Math.floor((playbackBaseOffsetSeconds + player.currentTime) * 1000),
      duration_ms: playbackItem.duration_ms ?? (Number.isFinite(player.duration) ? Math.floor(player.duration * 1000) : undefined),
      completed: false,
    }).catch((error) => console.warn('Failed to save playback position', error));
  });

  player.addEventListener('ended', () => {
    if (sourceDurationSeconds > playbackBaseOffsetSeconds + player.currentTime + 2) {
      void reportMediaError(player);
      return;
    }
    updatePlayButtons();
    showControls();
    void updatePlaybackProgress(playbackItem.id, {
      position_ms: playbackItem.duration_ms ?? Math.floor((playbackBaseOffsetSeconds + (Number.isFinite(player.duration) ? player.duration : 0)) * 1000),
      duration_ms: playbackItem.duration_ms ?? (Number.isFinite(player.duration) ? Math.floor(player.duration * 1000) : undefined),
      completed: true,
    }).catch((error) => console.warn('Failed to save completed playback', error));
  });

  updatePlayButtons();
  updateMuteButton();
  updatePipButton();
  updateTimeline();
  setPlayerLoading(player.readyState < player.HAVE_FUTURE_DATA);
  showControls();
  if (state.playbackSettings) {
    player.volume = state.playbackSettings.volume;
    player.muted = state.playbackSettings.muted;
  }
  const applyPlaybackSettings = (): void => {
    if (mediaInitialization.settingsApplied) return;
    mediaInitialization.settingsApplied = true;
    const saved = state.playbackSettings;
    if (!saved) return;
    player.volume = saved.volume;
    player.muted = saved.muted;
    for (const track of Array.from(player.textTracks)) track.mode = track.label === saved.subtitle ? 'showing' : 'disabled';
    if (saved.paused) player.pause();
  };
  if (player.readyState >= player.HAVE_METADATA) applyPlaybackSettings();
  const shouldStart = !mediaInitialization.started;
  mediaInitialization.started = true;
  if (state.playbackSettings?.paused || !shouldStart) return;
  void player.play().catch((error: unknown) => {
    console.warn('Autoplay after opening player was blocked or failed', error);
    updatePlayButtons();
    setPlayerLoading(false);
    showControls();
    if (error instanceof DOMException && error.name === 'NotSupportedError') void reportMediaError(player);
  });
}
