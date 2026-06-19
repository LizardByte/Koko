<script module lang="ts">
  // ItemBreadcrumbs stories. Reads $app/navigation (goto, mocked) + an item
  // prop. Hierarchy is exercised via the show→season→episode fixture below.
  import { defineMeta } from '@storybook/addon-svelte-csf';
  import ItemBreadcrumbs from './ItemBreadcrumbs.svelte';
  import { movieDetail } from '$lib/storybook/fixtures';
  import type { MediaItemDetail } from '$lib/api';

  const { Story } = defineMeta({
    title: 'Components/ItemBreadcrumbs',
    // No `component` — stories compose with explicit props; args are
    // decorator-only (preset/route) and not typed against Props.
    tags: ['autodocs'],
    args: { preset: 'empty' },
    parameters: {
      docs: {
        description: {
          component:
            'Hierarchy breadcrumb trail (Show / Season / current item). Empty when the item has no ancestors.',
        },
      },
    },
  });

  const episodeInHierarchy: MediaItemDetail = {
    ...movieDetail({
      id: 203,
      item_type: 'episode',
      display_title: 'Mock Episode',
      display_subtitle: 'Season 1 · Episode 1',
    }),
    hierarchy: [
      { id: 201, item_type: 'show', display_title: 'Mock Show', library_id: 2, parent_id: null, relative_path: '', media_kind: 'video', playable: false, child_count: 1, genres: [] },
      { id: 202, item_type: 'season', display_title: 'Season 1', library_id: 2, parent_id: 201, relative_path: '', media_kind: 'video', playable: false, child_count: 1, genres: [] },
    ],
  };
</script>

<Story name="Episode In Show" args={{ preset: 'empty' }} asChild>
  <ItemBreadcrumbs item={episodeInHierarchy} />
</Story>

<Story name="No Hierarchy" args={{ preset: 'empty' }} asChild>
  <ItemBreadcrumbs item={movieDetail()} />
</Story>
