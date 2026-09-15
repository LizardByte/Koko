<script module lang="ts">
  // HomeFeature stories. Props-driven (collection OR item). The item variant
  // reads the libraries store for the "from <library>" fallback, so preset
  // 'home' seeds it.
  import { defineMeta } from '@storybook/addon-svelte-csf';
  import HomeFeature from './HomeFeature.svelte';
  import { movieSummary } from '$lib/storybook/fixtures';
  import type { MediaCollectionSummary } from '$lib/api';

  const { Story } = defineMeta({
    title: 'Fragments/HomeFeature',
    // No `component` — stories compose the component with explicit props, so
    // args (preset/route) are decorator-only and not typed against Props.
    tags: ['autodocs'],
    args: { preset: 'home' },
    parameters: {
      docs: {
        description: {
          component:
            'Home hero spotlighting the preview item or collection. Backdrop uses the --home-feature-image CSS var with a left-edge mask; in mock mode it renders a solid base.',
        },
      },
    },
  });

  const collection: MediaCollectionSummary = {
    id: 'mock-collection',
    provider_id: 'tmdb',
    external_id: 'mock-collection',
    name: 'Mock Collection',
    item_ids: [101, 201],
    item_count: 2,
    overview: 'A curated mock collection for the home feature story.',
  };
</script>

<Story name="Item" args={{ preset: 'home' }} asChild>
  <HomeFeature item={movieSummary()} />
</Story>

<Story name="Collection" args={{ preset: 'home' }} asChild>
  <HomeFeature collection={collection} />
</Story>
