<script lang="ts">
  // PlayerControls — shared controls bar for MediaPlayer + TrailerPlayer
  // (Opportunity B — eliminates ~300 lines of duplicated vanilla markup).
  // All state is read from + written to the playback store reactively.
  //
  // Includes: progress slider, transport (seek ±10s + play/pause), volume +
  // mute, audio track menu (video only), PiP (video only), fullscreen, close.
  // Auto-hides after 3.2s of inactivity while playing (Opportunity F — $effect
  // timer with cleanup). Keyboard shortcuts via the playerShortcuts action
  // (Opportunity E). Accessibility: aria-live + range attributes (Opportunity G).
  import Icon from '../Icon.svelte';
  import AudioTrackMenu from './AudioTrackMenu.svelte';
  import { playback } from '$lib/stores';
  import { formatMediaTime } from '$lib/format';
  import { playerShortcuts } from '$lib/actions/playerShortcuts';
  import type { MediaAudioTrack } from '$lib/api';

  let {
    isVideo = true,
    audioTracks = [],
    onseek,
    onseekTo,
    onplaypause,
    onclose,
  }: {
    isVideo?: boolean;
    audioTracks?: MediaAudioTrack[];
    onseek?: (seconds: number) => void;
    onseekTo?: (seconds: number) => void;
    onplaypause?: () => void;
    onclose?: () => void;
  } = $props();

  // Progress slider position (0-1000, matching vanilla).
  let progressValue = $state(0);
  let isScrubbing = $state(false);

  // Fill percentages for the WebKit slider gradient.
  const progressFill = $derived(`${progressValue / 10}%`);
  const volumeFill = $derived(`${(playback.muted ? 0 : playback.volume) * 100}%`);

  // Sync slider from playback.currentTime unless scrubbing.
  $effect(() => {
    if (isScrubbing) return;
    const duration = playback.duration;
    progressValue = duration > 0 ? Math.min(1000, Math.max(0, (playback.currentTime / duration) * 1000)) : 0;
  });

  // Auto-hide controls (Opportunity F).
  let hideTimer: ReturnType<typeof setTimeout> | undefined;
  $effect(() => {
    // Reading these makes the effect re-run on any interaction/state change.
    void playback.isPlaying;
    void playback.currentTime;
    void isScrubbing;

    playback.controlsVisible = true;
    if (hideTimer) clearTimeout(hideTimer);
    hideTimer = setTimeout(() => {
      if (playback.isPlaying && !isScrubbing) {
        playback.controlsVisible = false;
      }
    }, 3200);

    return () => {
      if (hideTimer) clearTimeout(hideTimer);
    };
  });

  function showControls() {
    playback.controlsVisible = true;
    if (hideTimer) clearTimeout(hideTimer);
    hideTimer = setTimeout(() => {
      if (playback.isPlaying && !isScrubbing) playback.controlsVisible = false;
    }, 3200);
  }

  function togglePlayPause() {
    onplaypause?.();
    showControls();
  }

  function seekWithEscalation(direction: number) {
    const step = playback.nextSeekStep(direction);
    onseek?.(direction * step);
    showControls();
  }

  function onProgressInput() {
    isScrubbing = true;
    showControls();
  }

  function onProgressChange() {
    // Keep isScrubbing true during the seek — the $effect that syncs
    // progressValue from currentTime skips while scrubbing, so it won't
    // snap back to the old position. Reset after the seek completes
    // (next tick gives the timeupdate event time to fire with the new position).
    const duration = playback.duration;
    if (duration > 0 && onseekTo) {
      const targetSeconds = (Number(progressValue) / 1000) * duration;
      onseekTo(targetSeconds);
    }
    // Defer clearing isScrubbing so the $effect doesn't snap back before
    // the media element's seek completes.
    setTimeout(() => { isScrubbing = false; }, 100);
    showControls();
  }

  function toggleMute() {
    playback.muted = !playback.muted;
    showControls();
  }

  function volumeUp() {
    playback.volume = Math.min(1, playback.volume + 0.1);
    playback.muted = playback.volume === 0;
    showControls();
  }

  function volumeDown() {
    playback.volume = Math.max(0, playback.volume - 0.1);
    playback.muted = playback.volume === 0;
    showControls();
  }

  function toggleFullscreen() {
    const shell = document.querySelector('.player-shell');
    if (document.fullscreenElement) {
      document.exitFullscreen().catch(() => {});
    } else {
      shell?.requestFullscreen?.().catch(() => {});
    }
    showControls();
  }

  async function togglePiP() {
    const video = document.querySelector<HTMLVideoElement>('video');
    if (!video || !document.pictureInPictureEnabled) return;
    try {
      if (document.pictureInPictureElement) {
        await document.exitPictureInPicture();
      } else {
        if (document.fullscreenElement) await document.exitFullscreen();
        if (video.paused) await video.play();
        await video.requestPictureInPicture();
      }
    } catch {
      // PiP not available.
    }
    showControls();
  }

  // The active audio track, as an ARRAY index (0-based position in audioTracks).
  // The template compares this against the {#each} loop index.
  const activeAudioIndex = $derived.by(() => {
    // If the user explicitly switched, use their choice.
    if (playback.activeAudioStreamIndex !== undefined) {
      const pos = audioTracks.findIndex((t) => t.index === playback.activeAudioStreamIndex);
      if (pos >= 0) return pos;
    }
    // The server's session decides the initial audio stream — use that.
    if (playback.session?.audio_stream_index !== undefined) {
      const pos = audioTracks.findIndex((t) => t.index === playback.session!.audio_stream_index);
      if (pos >= 0) return pos;
    }
    // Fallback: the track marked 'default' in the container.
    const defaultPos = audioTracks.findIndex((t) => t.default);
    return defaultPos >= 0 ? defaultPos : 0;
  });
</script>

<svelte:window onmousemove={showControls} />

<div
  class="player-shell-controls"
  class:is-hidden={!playback.controlsVisible}
  use:playerShortcuts={{
    onPlayPause: togglePlayPause,
    onSeek: seekWithEscalation,
    onMute: toggleMute,
    onFullscreen: toggleFullscreen,
    onClose: () => onclose?.(),
    onVolumeUp: volumeUp,
    onVolumeDown: volumeDown,
  }}
  tabindex="-1"
  role="region"
  aria-label="Player controls"
>
  <!-- Top controls: title + close -->
  <div class="player-top-controls player-controls">
    <div class="player-title-block">
      <span class="eyebrow">Now playing</span>
      <h2>{playback.item?.display_title ?? playback.activeTrailer?.title ?? ''}</h2>
    </div>
    <div class="player-top-actions">
      {#if playback.isTranscoding}
        <span class="player-badge is-transcoding">Transcoding</span>
      {:else if playback.session}
        <span class="player-badge is-direct">Direct play</span>
      {/if}
      <button type="button" class="player-icon-button" title="Close" aria-label="Close player" onclick={() => onclose?.()}>
        <Icon name="x" size={20} />
      </button>
    </div>
  </div>

  <!-- Bottom controls: progress + transport -->
  <div class="player-bottom-controls player-controls">
    <input
      type="range"
      class="player-progress"
      min="0"
      max="1000"
      step="1"
      bind:value={progressValue}
      style="--slider-fill: {progressFill}"
      aria-label="Playback position"
      aria-valuemin={0}
      aria-valuemax={1000}
      oninput={onProgressInput}
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
        <input
          type="range"
          class="player-volume"
          min="0"
          max="1"
          step="0.01"
          value={playback.muted ? 0 : playback.volume}
          style="--slider-fill: {volumeFill}"
          aria-label="Volume"
          oninput={(e) => { playback.volume = Number((e.currentTarget as HTMLInputElement).value); playback.muted = playback.volume === 0; }}
        />
        {#if isVideo && audioTracks.length > 1}
          <AudioTrackMenu {audioTracks} activeIndex={activeAudioIndex} onselect={(i) => playback.switchAudioTrack(audioTracks[i].index)} />
        {/if}
        {#if isVideo}
          <button type="button" class="player-icon-button" title="Picture in picture" aria-label="Picture in picture" onclick={togglePiP}>
            <Icon name="picture-in-picture" size={20} />
          </button>
        {/if}
        <button type="button" class="player-icon-button" title="Fullscreen" aria-label="Fullscreen" onclick={toggleFullscreen}>
          <Icon name="maximize" size={20} />
        </button>
      </div>
    </div>
  </div>
</div>
