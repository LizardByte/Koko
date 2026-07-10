<script module lang="ts">
  // Player stories — the player UI. Store-driven (playback store). The mock
  // API returns 501 for stream endpoints, so the media player shows its
  // loading/error state. The controls UI is fully testable.
  import { defineMeta } from '@storybook/addon-svelte-csf';
  import PlayerControls from './PlayerControls.svelte';
  import { playback } from '$lib/stores';
  import { movieDetail } from '$lib/storybook/fixtures';

  const { Story } = defineMeta({
    title: 'Screens/Player',
    tags: ['autodocs'],
    args: { preset: 'item-movie' },
    parameters: {
      docs: {
        description: {
          component:
            'Player overlay: HTML5 video/audio media player + YouTube trailer overlay + ambient theme song. Store-driven via the playback store. The mock API returns 501 for stream endpoints, so the media player shows its error state in Storybook — the controls UI is fully testable.',
        },
      },
    },
  });

  // Seed playback state for the controls story.
  const item = movieDetail();
  playback.item = item;
  playback.duration = (item.duration_ms ?? 6300000) / 1000;
  playback.currentTime = 1260;
  playback.isPlaying = true;
  playback.volume = 0.8;
</script>

<!-- PlayerControls standalone — shows the controls bar in a dark frame -->
<Story name="Controls Bar (Playing)" args={{ preset: 'item-movie' }} asChild>
  <div style="position:relative;width:100%;height:400px;background:#000;border-radius:16px;overflow:hidden;">
    <PlayerControls
      isVideo={true}
      audioTracks={item.audio_tracks ?? []}
      onseek={() => {}}
      onplaypause={() => { playback.isPlaying = !playback.isPlaying; }}
      onclose={() => {}}
    />
  </div>
</Story>

<Story name="Controls Bar (Paused)" args={{ preset: 'item-movie' }} asChild>
  <div style="position:relative;width:100%;height:400px;background:#000;border-radius:16px;overflow:hidden;">
    <PlayerControls
      isVideo={true}
      audioTracks={item.audio_tracks ?? []}
      onseek={() => {}}
      onplaypause={() => {}}
      onclose={() => {}}
    />
  </div>
</Story>
