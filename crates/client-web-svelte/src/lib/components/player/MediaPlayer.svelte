<script lang="ts">
  // MediaPlayer — the HTML5 video/audio player overlay. Renders the media
  // element with subtitle tracks + poster art (audio mode), binds element
  // state to the playback store, reports progress via $effect (Opportunity D),
  // and cleans up the session on unmount (Opportunity F).
  //
  // Replaces vanilla renderMediaPlayerOverlay + bindPlayerProgress (~560 lines)
  // with ~120 lines of reactive Svelte.
  import Icon from '../Icon.svelte';
  import { getArtworkUrl, resolveApiUrl } from '$lib/api';
  import { playback } from '$lib/stores';
  import PlayerControls from './PlayerControls.svelte';

  let mediaElement = $state<HTMLMediaElement | undefined>(undefined);

  // The stream URL from the playback store.
  const streamUrl = $derived(playback.streamUrl);
  const isAudio = $derived(playback.isAudio);
  const posterUrl = $derived(
    playback.item ? getArtworkUrl(playback.item.id, 'poster', playback.item.artwork_updated_at) : undefined,
  );
  const backdropUrl = $derived(
    playback.item ? getArtworkUrl(playback.item.id, 'backdrop', playback.item.artwork_updated_at) : undefined,
  );
  const subtitleTracks = $derived(playback.item?.subtitle_tracks ?? []);
  const audioTracks = $derived(playback.item?.audio_tracks ?? []);

  // Initial seek for direct-play (seek to startMs on metadata loaded).
  let hasAppliedInitialSeek = $state(false);

  // --- Bind media element state to the playback store ---

  $effect(() => {
    const el = mediaElement;
    if (!el) return;
    // Capture in a const so closures see the non-null type.
    const media = el;

    function onTimeUpdate() {
      playback.currentTime = media.currentTime;
      // Progress reporting (Opportunity D) — every 15s boundary.
      if (playback.item) {
        playback.reportProgress(
          playback.item.id,
          media.currentTime,
          playback.item.duration_ms,
        );
      }
    }
    function onDurationChange() {
      // Pin duration from item metadata when available — mirrors vanilla's
      // playbackDurationSeconds() (playbackController.ts:1033-1041) which
      // prefers sourceDurationSeconds over player.duration. During transcoding,
      // the fragmented-MP4 stream reports a growing per-fragment duration (no
      // fixed total in the moov atom), so the element's duration is wrong.
      // Only fall back to media.duration when item metadata is unknown.
      const metaDuration = (playback.item?.duration_ms ?? 0) / 1000;
      playback.duration = metaDuration > 0
        ? metaDuration
        : (Number.isFinite(media.duration) && media.duration > 0 ? media.duration : 0);
    }
    function onPlay() {
      playback.isPlaying = true;
    }
    function onPause() {
      playback.isPlaying = false;
    }
    function onVolumeChange() {
      playback.volume = media.volume;
      playback.muted = media.muted || media.volume === 0;
    }
    function onWaiting() {
      playback.isLoading = media.readyState < media.HAVE_FUTURE_DATA;
    }
    function onPlaying() {
      playback.isLoading = false;
    }
    function onCanPlay() {
      playback.isLoading = false;
      applyInitialSeek();
    }
    function onLoadedMetadata() {
      applyInitialSeek();
    }
    function onError() {
      playback.hasError = true;
      playback.isLoading = false;
    }
    function onEnded() {
      playback.isPlaying = false;
      if (playback.item) {
        playback.reportCompleted(
          playback.item.id,
          playback.item.duration_ms ?? (Number.isFinite(media.duration) ? Math.floor(media.duration * 1000) : undefined),
        );
      }
    }
    function onLeavePiP() {
      playback.isPictureInPicture = false;
    }
    function onEnterPiP() {
      playback.isPictureInPicture = true;
    }

    function applyInitialSeek() {
      if (hasAppliedInitialSeek || playback.isTranscoding) return;
      if (media.readyState < media.HAVE_METADATA) return;
      const targetSeconds = playback.startMs / 1000;
      if (targetSeconds > 0) {
        try {
          media.currentTime = Math.min(targetSeconds, media.duration - 1);
          hasAppliedInitialSeek = true;
        } catch {
          // Some browsers reject seeks before readyState is sufficient.
        }
      } else {
        hasAppliedInitialSeek = true;
      }
    }

    media.addEventListener('timeupdate', onTimeUpdate);
    media.addEventListener('durationchange', onDurationChange);
    media.addEventListener('play', onPlay);
    media.addEventListener('pause', onPause);
    media.addEventListener('volumechange', onVolumeChange);
    media.addEventListener('waiting', onWaiting);
    media.addEventListener('stalled', onWaiting);
    media.addEventListener('playing', onPlaying);
    media.addEventListener('canplay', onCanPlay);
    media.addEventListener('loadedmetadata', onLoadedMetadata);
    media.addEventListener('error', onError);
    media.addEventListener('ended', onEnded);
    media.addEventListener('leavepictureinpicture', onLeavePiP);
    media.addEventListener('enterpictureinpicture', onEnterPiP);

    // Attempt autoplay.
    media.play().catch(() => {
      // Autoplay blocked — user needs to click play.
      playback.isPlaying = false;
      playback.isLoading = false;
    });

    return () => {
      media.removeEventListener('timeupdate', onTimeUpdate);
      media.removeEventListener('durationchange', onDurationChange);
      media.removeEventListener('play', onPlay);
      media.removeEventListener('pause', onPause);
      media.removeEventListener('volumechange', onVolumeChange);
      media.removeEventListener('waiting', onWaiting);
      media.removeEventListener('stalled', onWaiting);
      media.removeEventListener('playing', onPlaying);
      media.removeEventListener('canplay', onCanPlay);
      media.removeEventListener('loadedmetadata', onLoadedMetadata);
      media.removeEventListener('error', onError);
      media.removeEventListener('ended', onEnded);
      media.removeEventListener('leavepictureinpicture', onLeavePiP);
      media.removeEventListener('enterpictureinpicture', onEnterPiP);
    };
  });

  // Opportunity F: cleanup on unmount — delete the session.
  $effect(() => {
    return () => {
      // Only delete if we still own the session (not if close() already did it).
      if (playback.mode === 'media' && playback.session) {
        playback.close();
      }
    };
  });

  // --- Sync store → media element (volume, muted) ---

  $effect(() => {
    const el = mediaElement;
    if (!el) return;
    // Write store values to the element. The element's volumechange event
    // updates the store (read direction); this $effect handles the write
    // direction so the volume slider actually controls the audio.
    el.volume = playback.volume;
    el.muted = playback.muted;
  });

  // --- Seek handler for controls ---

  /** Relative seek (±seconds) — used by skip buttons + keyboard shortcuts. */
  function seek(deltaSeconds: number) {
    const el = mediaElement;
    if (!el) return;
    if (playback.isTranscoding) {
      // For transcoded streams, update startMs + reload the stream.
      playback.startMs = Math.max(0, Math.floor((el.currentTime + deltaSeconds) * 1000));
      hasAppliedInitialSeek = false;
      el.load();
    } else {
      // Direct-play: seek client-side.
      el.currentTime = Math.max(0, el.currentTime + deltaSeconds);
    }
  }

  /** Absolute seek (to a specific second) — used by the progress slider. */
  function seekTo(targetSeconds: number) {
    const el = mediaElement;
    if (!el) return;
    if (playback.isTranscoding) {
      playback.startMs = Math.max(0, Math.floor(targetSeconds * 1000));
      hasAppliedInitialSeek = false;
      el.load();
    } else {
      el.currentTime = Math.max(0, Math.min(targetSeconds, el.duration || targetSeconds));
    }
  }

  function togglePlayPause() {
    const el = mediaElement;
    if (!el) return;
    if (el.paused) {
      el.play().catch(() => {});
    } else {
      el.pause();
    }
  }
</script>

{#if playback.item && streamUrl}
  <div class="player-overlay media-player-overlay">
    <div
      class="player-shell media-player-shell"
      class:audio-player-shell={isAudio}
      class:video-player-shell={!isAudio}
      class:is-controls-visible={playback.controlsVisible}
      class:is-controls-hidden={!playback.controlsVisible}
      class:is-media-loading={playback.isLoading}
      class:has-media-error={playback.hasError}
      class:is-picture-in-picture={playback.isPictureInPicture}
      tabindex="-1"
      style={backdropUrl ? `--player-backdrop-image: url('${backdropUrl}');` : ''}
    >
      {#if !isAudio}
        <!-- Video mode -->
        <video
          bind:this={mediaElement}
          autoplay
 preload="metadata"
          playsinline
          src={streamUrl}
          poster={posterUrl}
        >
          {#each subtitleTracks as track (track.index)}
            <track kind="subtitles" label={track.label} src={resolveApiUrl(track.url)} />
          {/each}
        </video>
      {:else}
        <!-- Audio mode -->
        <div class="audio-player-backdrop" aria-hidden="true"></div>
        <div class="audio-player-art" class:has-image={Boolean(posterUrl)}>
          {#if posterUrl}
            <img src={posterUrl} alt="" />
          {:else}
            <span class="audio-player-art-icon"><Icon name="music" size={48} /></span>
          {/if}
        </div>
        <audio bind:this={mediaElement} autoplay preload="metadata" src={streamUrl}></audio>
      {/if}

      <!-- Loading indicator (Opportunity G: aria-live) -->
      <div class="player-loading-indicator" aria-live="polite">
        <span class="loading-spinner player-loading-spinner" aria-hidden="true"></span>
      </div>

      <!-- Error indicator (Opportunity G: aria-live) -->
      <div class="player-error-indicator" aria-live="polite">
        <strong>Playback could not start</strong>
        <span>Try another audio track or start playback again.</span>
      </div>

      <!-- Click-to-toggle idle area -->
      <div class="player-idle-hit-area" aria-hidden="true" onclick={togglePlayPause}></div>

      <!-- Shared controls (Opportunity B) -->
      <PlayerControls
        isVideo={!isAudio}
        {audioTracks}
        onseek={seek}
        onseekTo={seekTo}
        onplaypause={togglePlayPause}
        onclose={() => playback.close()}
      />
    </div>
  </div>
{/if}
