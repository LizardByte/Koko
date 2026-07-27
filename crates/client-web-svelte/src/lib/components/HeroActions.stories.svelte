<script module lang="ts">
  // HeroActions stories — the action-button row of the item hero (Resume, Play,
  // Trailer, Theme, Back). Extracted from SectionHero. Playback clicks are
  // stubs (playbackController spike) — they surface a ui.error message.
  import { defineMeta } from '@storybook/addon-svelte-csf';
  import HeroActions from './HeroActions.svelte';
  import { movieDetail } from '$lib/storybook/fixtures';

  const { Story } = defineMeta({
    title: 'Components/HeroActions',
    tags: ['autodocs'],
    args: { preset: 'item-movie' },
    parameters: {
      docs: {
        description: {
            component: 'Action-button row for the item hero: Resume (if in-progress), Play now / Start over, Play target, Restart target, Trailer, Theme, Back. Mirrors vanilla renderSelectedItemActions. Consumes the item/ui stores.',
        },
      },
    },
  });

  const playable = movieDetail({ playable: true, playback_position_ms: 1_260_000, playback_duration_ms: 6_300_000 });
  const fresh = movieDetail({ display_title: 'Fresh Movie' });
  const withTrailer = movieDetail({ display_title: 'Movie With Trailer', trailer_url: 'mock://trailer', trailer_title: 'Official Trailer' });
</script>

<Story name="Resume + Start Over" args={{ preset: 'item-movie' }} asChild>
  <div class="item-summary" style="display:flex;flex-direction:column;gap:0.8rem;">
    <HeroActions itemValue={playable} />
  </div>
</Story>

<Story name="Fresh (Play Now)" args={{ preset: 'item-movie' }} asChild>
  <div class="item-summary" style="display:flex;flex-direction:column;gap:0.8rem;">
    <HeroActions itemValue={fresh} />
  </div>
</Story>

<Story name="With Trailer + Theme" args={{ preset: 'item-movie' }} asChild>
  <div class="item-summary" style="display:flex;flex-direction:column;gap:0.8rem;">
    <HeroActions itemValue={withTrailer} />
  </div>
</Story>
