<script lang="ts">
  // PlayerOverlay — top-level overlay rendered in +layout.svelte. Reads
  // playback.mode to decide which player to render. Always renders
  // ThemeSongPlayer (ambient, hidden when player/trailer is open).
  import MediaPlayer from './MediaPlayer.svelte';
  import TrailerPlayer from './TrailerPlayer.svelte';
  import ThemeSongPlayer from './ThemeSongPlayer.svelte';
  import { playback } from '$lib/stores';
</script>

{#if playback.mode === 'media'}
  <MediaPlayer />
{:else if playback.mode === 'trailer'}
  <TrailerPlayer />
{/if}

<!-- Ambient theme song (always rendered; self-suppresses when player is open) -->
<ThemeSongPlayer />
