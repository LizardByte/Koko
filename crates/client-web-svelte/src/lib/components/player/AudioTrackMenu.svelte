<script lang="ts">
  // AudioTrackMenu — dropdown for selecting an audio track. When a non-default
  // track is selected, the playback store re-creates the session with the new
  // audio_stream_index (server remuxes/transcodes).
  import Icon from '../Icon.svelte';
  import type { MediaAudioTrack } from '$lib/api';

  let {
    audioTracks,
    activeIndex,
    onselect,
  }: {
    audioTracks: MediaAudioTrack[];
    activeIndex: number;
    onselect: (index: number) => void;
  } = $props();

  let isOpen = $state(false);

  function toggle() {
    isOpen = !isOpen;
  }

  function select(index: number) {
    if (index !== activeIndex) {
      onselect(index);
    }
    isOpen = false;
  }
</script>

<div class="player-menu-shell">
  <button
    type="button"
    class="player-icon-button"
    title="Audio tracks"
    aria-label="Audio tracks"
    aria-expanded={isOpen}
    onclick={toggle}
  >
    <Icon name="languages" size={18} />
  </button>
  {#if isOpen}
    <div class="player-track-menu">
      {#each audioTracks as track, i (track.index)}
        <button
          type="button"
          class="player-track-option"
          class:active={i === activeIndex}
          onclick={() => select(i)}
        >
          <span>{track.label}</span>
          {#if track.language}<small>{track.language}</small>{/if}
        </button>
      {/each}
    </div>
  {/if}
</div>
