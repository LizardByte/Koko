<script module>
  // BrowseDetail stories. Reads the catalog store for collections + library
  // items (category/collection resolution). preset 'home' seeds both.
  // Uses asChild to mount the component explicitly (the args-driven path
  // doesn't forward a children snippet reliably to global decorators in
  // Svelte CSF v5 — asChild is the documented composed-story pattern).
  import { defineMeta } from '@storybook/addon-svelte-csf';
  import BrowseListing from './BrowseListing.svelte';

  const { Story } = defineMeta({
    title: 'Fragments/BrowseListing',
    // No `component` — stories use asChild to mount with explicit props.
    tags: ['autodocs'],
    args: { preset: 'home' },
    parameters: {
      docs: {
        description: {
          component:
            'Collection/category/playlist detail (hero + item grid). Resolves data from the catalog store + route params. Routed at /items/<kind>/<key> and /libraries/:id/items/<kind>/<key>.\n\n' +
            '> ⚠️ **Store-driven component.** BrowseDetail takes only `kind`/`key`/`libraryId` props — ' +
            'all its data (collections, library items, loading state) comes from the `catalog` store, ' +
            'seeded by the `preset` arg (see `.storybook/decorators/withStores.ts`). That’s why the ' +
            'controls panel shows only those three props: switch the `preset` arg to drive different ' +
            'fixture data. **TODO:** thread collection + items + loading as props so this can be ' +
            'previewed with arbitrary data without store seeding.',
        },
      },
    },
  });
</script>

<Story name="Collection" args={{ preset: 'home' }} asChild>
  <BrowseListing kind="collection" key="mock-collection" />
</Story>

<Story name="Category" args={{ preset: 'home' }} asChild>
  <BrowseListing kind="category" key="Action" />
</Story>

<Story name="Playlist" args={{ preset: 'home' }} asChild>
  <BrowseListing kind="playlist" key="Playlists" />
</Story>
