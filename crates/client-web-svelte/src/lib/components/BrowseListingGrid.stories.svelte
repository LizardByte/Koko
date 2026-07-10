<script module lang="ts">
  // BrowseListingGrid stories — the items panel of a browse-listing page
  // (heading, loading state, per-kind empty state, MediaCard grid).
  // Presentational; items + loading + kind via props.
  import { defineMeta } from '@storybook/addon-svelte-csf';
  import BrowseListingGrid from './BrowseListingGrid.svelte';
  import { movieSummary, showSummary } from '$lib/storybook/fixtures';
  import type { MediaItemSummary } from '$lib/api';

  const { Story } = defineMeta({
    title: 'Fragments/BrowseListingGrid',
    tags: ['autodocs'],
    args: { preset: 'home' },
    parameters: {
      docs: {
        description: {
            component: 'Items panel of a browse-listing page: heading, loading state, per-kind empty state, or the MediaCard grid. Presentational — items + loading + kind passed as props by BrowseListing.',
        },
      },
    },
  });

  const items: MediaItemSummary[] = [movieSummary(), showSummary(), movieSummary({ id: 102, display_title: 'Another Movie' })];
</script>

<Story name="Populated" args={{ preset: 'home' }} asChild>
  <BrowseListingGrid {items} loading={false} kind="collection" />
</Story>

<Story name="Loading" args={{ preset: 'home' }} asChild>
  <BrowseListingGrid items={[]} loading={true} kind="collection" />
</Story>

<Story name="Empty Collection" args={{ preset: 'home' }} asChild>
  <BrowseListingGrid items={[]} loading={false} kind="collection" />
</Story>

<Story name="Empty Category" args={{ preset: 'home' }} asChild>
  <BrowseListingGrid items={[]} loading={false} kind="category" />
</Story>

<Story name="Empty Playlist" args={{ preset: 'home' }} asChild>
  <BrowseListingGrid items={[]} loading={false} kind="playlist" />
</Story>
