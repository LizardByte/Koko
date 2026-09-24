<script lang="ts">
  // ThemeSongPlayer — ambient background theme song. Opportunity C: audio-first.
  // If the theme song URL is a direct audio file, uses a hidden <audio> element
  // (no YouTube API overhead). If it's a YouTube URL, uses YouTubeIframe with
  // height/width 0. Pauses when the player/trailer overlay is active.
  //
  // Replaces vanilla ensureThemeSongYouTubePlayer + syncThemeSongPlayer (~120 lines)
  // with ~40 lines of reactive Svelte.
  import { playback } from '$lib/stores';
  import YouTubeIframe from './YouTubeIframe.svelte';
  import { resolveApiUrl } from '$lib/api';

  const source = $derived(playback.themeSongSource);

  // The player/trailer being open suppresses the theme song.
  const shouldPlay = $derived(source !== undefined && !playback.isOpen);

  let audioElement = $state<HTMLAudioElement | undefined>(undefined);

  // Play/pause the audio element based on shouldPlay.
  $effect(() => {
    const el = audioElement;
    if (!el || source?.kind !== 'audio') return;
    if (shouldPlay) {
      el.play().catch(() => {
        // Autoplay may be blocked until user interaction.
      });
    } else {
      el.pause();
    }
  });

  // Cleanup on unmount.
  $effect(() => {
    return () => {
      audioElement?.pause();
    };
  });
</script>

{#if source && shouldPlay}
  {#if source.kind === 'audio'}
    <!-- Audio-first: plain <audio>, no YouTube API overhead (Opportunity C) -->
    <audio
      bind:this={audioElement}
      src={resolveApiUrl(source.src)}
      loop
      preload="auto"
      class="theme-song-audio"
    ></audio>
  {:else if source.kind === 'youtube'}
    <!-- YouTube fallback: hidden IFrame player -->
    <div class="theme-song-layer" aria-hidden="true">
      <YouTubeIframe videoId={source.videoId} autoplay loop controls={false} />
    </div>
  {/if}
{/if}
