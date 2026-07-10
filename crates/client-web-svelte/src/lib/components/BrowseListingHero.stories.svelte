<script module lang="ts">
  // BrowseListingHero stories — the top banner of a browse-listing page
  // (collection/category/playlist). Presentational; all data via props.
  import { defineMeta } from '@storybook/addon-svelte-csf';
  import BrowseListingHero from './BrowseListingHero.svelte';

  const { Story } = defineMeta({
    title: 'Fragments/BrowseListingHero',
    tags: ['autodocs'],
    args: { preset: 'empty' },
    parameters: {
      docs: {
        description: {
            component: 'Top banner of a browse-listing page: poster/initial, eyebrow, title, item-count tag, overview, Back button. Presentational — data passed as props by BrowseListing.',
        },
      },
    },
  });

  const noop = () => {};
</script>

<Story name="Collection" args={{ preset: 'empty' }} asChild>
  <BrowseListingHero
    eyebrow="Collection"
    title="Marvel Cinematic Universe"
    itemCount={12}
    overview="A curated collection of MCU films in release order."
    onBack={noop}
  />
</Story>

<Story name="Category" args={{ preset: 'empty' }} asChild>
  <BrowseListingHero
    eyebrow="Genre"
    title="Action"
    itemCount={47}
    overview="All action titles in this library."
    onBack={noop}
  />
</Story>

<Story name="Playlist" args={{ preset: 'empty' }} asChild>
  <BrowseListingHero
    eyebrow="Playlist"
    title="Road Trip"
    itemCount={0}
    overview="No playlist items are available yet."
    onBack={noop}
  />
</Story>

<Story name="With Poster" args={{ preset: 'empty' }} asChild>
  <BrowseListingHero
    eyebrow="Collection"
    title="With Artwork"
    itemCount={3}
    overview="A collection with custom artwork."
    posterUrl="https://picsum.photos/seed/koko-collection/300/450"
    onBack={noop}
  />
</Story>
