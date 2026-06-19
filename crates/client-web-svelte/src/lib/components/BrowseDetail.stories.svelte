<script module>
  // BrowseDetail stories. Reads the catalog store for collections + library
  // items (category/collection resolution). preset 'home' seeds both.
  // Uses asChild to mount the component explicitly (the args-driven path
  // doesn't forward a children snippet reliably to global decorators in
  // Svelte CSF v5 — asChild is the documented composed-story pattern).
  import { defineMeta } from '@storybook/addon-svelte-csf';
  import BrowseDetail from './BrowseDetail.svelte';

  const { Story } = defineMeta({
    title: 'Fragments/BrowseDetail',
    // No `component` — stories use asChild to mount with explicit props.
    tags: ['autodocs'],
    args: { preset: 'home' },
    parameters: {
      docs: {
        description: {
          component:
            'Collection/category/playlist detail (hero + item grid). Resolves data from the catalog store + route params. Routed at /items/<kind>/<key> and /libraries/:id/items/<kind>/<key>.',
        },
      },
    },
  });
</script>

<Story name="Collection" args={{ preset: 'home' }} asChild>
  <BrowseDetail kind="collection" key="mock-collection" />
</Story>

<Story name="Category" args={{ preset: 'home' }} asChild>
  <BrowseDetail kind="category" key="Action" />
</Story>

<Story name="Playlist" args={{ preset: 'home' }} asChild>
  <BrowseDetail kind="playlist" key="Playlists" />
</Story>
