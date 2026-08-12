<script module lang="ts">
  // FactList stories — the technical-facts grid (codec, resolution, duration)
  // at the bottom of the item hero. Extracted from SectionHero.
  import { defineMeta } from '@storybook/addon-svelte-csf';
  import FactList from './FactList.svelte';
  import { movieDetail } from '$lib/storybook/fixtures';

  const { Story } = defineMeta({
    title: 'Components/FactList',
    tags: ['autodocs'],
    args: { preset: 'item-movie' },
    parameters: {
      docs: {
        description: {
            component: 'Technical-facts grid (codec, resolution, duration, etc.) derived from the item via selectedItemTechnicalFacts. Mirrors vanilla renderSelectedItemFactList.',
        },
      },
    },
  });

  const movie = movieDetail();
</script>

<Story name="Movie" args={{ preset: 'item-movie' }} asChild>
  <FactList itemValue={movie} />
</Story>
