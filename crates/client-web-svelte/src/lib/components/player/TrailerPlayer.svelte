<script lang="ts">
  // TrailerPlayer — YouTube trailer overlay. Uses YouTubeIframe + the shared
  // PlayerControls (Opportunity B). Shows trailer title, external-link button,
  // and close button. YouTube chrome hidden via CSS mask.
  //
  // Replaces vanilla renderTrailerOverlay + bindTrailerPlayer (~400 lines)
  // with ~80 lines of reactive Svelte.
  import Icon from '../Icon.svelte';
  import YouTubeIframe from './YouTubeIframe.svelte';
  import { playback } from '$lib/stores';
  import { formatMediaTime } from '$lib/format';
  import { extractYouTubeVideoId, buildYouTubeWatchUrl } from '$lib/youtube';
  import type { YouTubePlayer } from '$lib/playerTypes';
  import { YOUTUBE_PLAYER_STATE } from '$lib/playerTypes';

  const trailer = $derived(playback.activeTrailer);
  const videoId = $derived(trailer ? extractYouTubeVideoId(trailer.url) : undefined);
  const externalUrl = $derived(trailer ? buildYouTubeWatchUrl(trailer.url) : undefined);

  let ytPlayer: YouTubePlayer | undefined = $state(undefined);
  let progressValue = $state(0);
  let isScrubbing = $state(false);
  let progressTimer: ReturnType<typeof setInterval> | undefined;

  function onReady(player: YouTubePlayer) {
    ytPlayer = player;
    playback.isLoading = false;
    // Poll for progress updates (vanilla uses 500ms interval).
    if (progressTimer) clearInterval(progressTimer);
    progressTimer = setInterval(updateFromPlayer, 500);
  }

  function onStateChange(player: YouTubePlayer) {
    const state = player.getPlayerState();
    playback.isPlaying = state === YOUTUBE_PLAYER_STATE.playing;
    playback.isLoading = state === YOUTUBE_PLAYER_STATE.buffering || state === YOUTUBE_PLAYER_STATE.cued;
    updateFromPlayer();
  }

  function updateFromPlayer() {
    if (!ytPlayer) return;
    const duration = ytPlayer.getDuration();
    const currentTime = ytPlayer.getCurrentTime();
    playback.duration = duration;
    playback.currentTime = currentTime;
    playback.volume = ytPlayer.getVolume() / 100;
    playback.muted = ytPlayer.isMuted();
    if (!isScrubbing) {
      progressValue = duration > 0 ? Math.min(1000, Math.max(0, (currentTime / duration) * 1000)) : 0;
    }
  }

  // Cleanup on unmount.
  $effect(() => {
    return () => {
      if (progressTimer) clearInterval(progressTimer);
    };
  });

  function togglePlayPause() {
    if (!ytPlayer) return;
    if (ytPlayer.getPlayerState() === YOUTUBE_PLAYER_STATE.playing) {
      ytPlayer.pauseVideo();
    } else {
      ytPlayer.playVideo();
    }
  }

  function seek(deltaSeconds: number) {
    if (!ytPlayer) return;
    const duration = ytPlayer.getDuration();
    const currentTime = ytPlayer.getCurrentTime();
    const target = duration > 0
      ? Math.min(duration, Math.max(0, currentTime + deltaSeconds))
      : Math.max(0, currentTime + deltaSeconds);
    ytPlayer.seekTo(target, true);
  }

  function seekWithEscalation(direction: number) {
    const step = playback.nextSeekStep(direction);
    seek(direction * step);
  }

  function onProgressChange() {
    if (!ytPlayer) return;
    const duration = ytPlayer.getDuration();
    if (duration > 0) {
      ytPlayer.seekTo((Number(progressValue) / 1000) * duration, true);
    }
    isScrubbing = false;
    updateFromPlayer();
  }

  function toggleMute() {
    if (!ytPlayer) return;
    if (ytPlayer.isMuted()) {
      ytPlayer.unMute();
    } else {
      ytPlayer.mute();
    }
    updateFromPlayer();
  }

  function toggleFullscreen() {
    const shell = document.querySelector('.player-shell');
    if (document.fullscreenElement) {
      document.exitFullscreen().catch(() => {});
    } else {
      shell?.requestFullscreen?.().catch(() => {});
    }
  }
</script>

{#if trailer && videoId}
  <div class="player-overlay trailer-overlay">
    <div
      class="player-shell trailer-shell"
      class:is-controls-visible={playback.controlsVisible}
      class:is-controls-hidden={!playback.controlsVisible}
      class:is-media-loading={playback.isLoading}
      data-trailer-video-id={videoId}
      tabindex="-1"
      role="region"
      aria-label={trailer.title}
    >
      <div class="trailer-frame-shell" aria-label={trailer.title}>
        <YouTubeIframe {videoId} onready={onReady} onstatechange={onStateChange} />
      </div>
      <div class="trailer-youtube-chrome-mask" aria-hidden="true"></div>

      <!-- Loading/error -->
      <div class="player-loading-indicator" aria-live="polite">
        <span class="loading-spinner player-loading-spinner" aria-hidden="true"></span>
      </div>

      <!-- Top controls -->
      <div class="player-top-controls player-controls">
        <div class="player-title-block">
          <span class="eyebrow">{trailer.label ?? 'Trailer'}</span>
          <h2>{trailer.title}</h2>
        </div>
        <div class="player-top-actions">
          {#if externalUrl}
            <a class="button-link secondary-button" href={externalUrl} target="_blank" rel="noreferrer">
              <span class="button-content"><span class="button-icon"><Icon name="arrow-right" size={16} /></span>Open on YouTube</span>
            </a>
          {/if}
          <button type="button" class="player-icon-button" title="Close trailer" aria-label="Close trailer" onclick={() => playback.closeTrailer()}>
            <Icon name="x" size={20} />
          </button>
        </div>
      </div>

      <!-- Bottom controls -->
      <div class="player-bottom-controls player-controls">
        <input
          type="range"
          class="player-progress"
          min="0"
          max="1000"
          step="1"
          bind:value={progressValue}
          aria-label="Trailer position"
          oninput={() => { isScrubbing = true; }}
          onchange={onProgressChange}
        />
        <div class="player-control-row">
          <div class="player-control-cluster player-time-cluster">
            <span class="player-time">
              <span>{formatMediaTime(playback.currentTime)}</span>
              <span>/</span>
              <span>{formatMediaTime(playback.duration)}</span>
            </span>
          </div>
          <div class="player-control-cluster player-transport-cluster">
            <button type="button" class="player-icon-button" title="Back 10 seconds" aria-label="Back 10 seconds" onclick={() => seekWithEscalation(-1)}>
              <Icon name="skip-back" size={20} />
            </button>
            <button type="button" class="player-icon-button player-primary-button" title={playback.isPlaying ? 'Pause' : 'Play'} aria-label={playback.isPlaying ? 'Pause' : 'Play'} onclick={togglePlayPause}>
              <Icon name={playback.isPlaying ? 'pause' : 'play'} size={24} />
            </button>
            <button type="button" class="player-icon-button" title="Forward 10 seconds" aria-label="Forward 10 seconds" onclick={() => seekWithEscalation(1)}>
              <Icon name="skip-forward" size={20} />
            </button>
          </div>
          <div class="player-control-cluster player-tool-cluster">
            <button type="button" class="player-icon-button" title={playback.muted ? 'Unmute' : 'Mute'} aria-label={playback.muted ? 'Unmute' : 'Mute'} onclick={toggleMute}>
              <Icon name={playback.muted ? 'volume-x' : 'volume-2'} size={20} />
            </button>
            <button type="button" class="player-icon-button" title="Fullscreen" aria-label="Fullscreen" onclick={toggleFullscreen}>
              <Icon name="maximize" size={20} />
            </button>
          </div>
        </div>
      </div>
    </div>
  </div>
{/if}
