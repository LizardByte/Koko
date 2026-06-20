<script module lang="ts">
  // HomeContent stories — the home page body: feature spotlight, shelves of
  // MediaCards, collections rail, and search results. Store-driven: reads
  // catalog.home / catalog.searchQuery / catalog.libraryItems, seeded by the
  // `preset` arg (default 'home').
  import { defineMeta } from '@storybook/addon-svelte-csf';
  import HomeContent from './HomeContent.svelte';

  const { Story } = defineMeta({
    title: 'Fragments/HomeContent',
    tags: ['autodocs'],
    args: { preset: 'home' },
    parameters: {
      docs: {
        description: {
          component:
            'Home page body: HomeFeature spotlight, horizontal Shelves of MediaCards, collections rail, and a search-results view when a query is active. Takes only `libraryId?` — all data comes from the `catalog` store, seeded by the `preset` arg (see withStores decorator).\n\n> ⚠️ **Store-driven.** The content (shelves, collections, search) is entirely from the catalog store. Switch the `preset` arg to drive different fixture data.',
        },
      },
    },
  });
</script>

<Story name="Home (All Libraries)" args={{ preset: 'home' }} asChild>
  <HomeContent />
</Story>

<Story name="Home (Specific Library)" args={{ preset: 'home', route: '/libraries/2' }} asChild>
  <HomeContent libraryId={2} />
</Story>
