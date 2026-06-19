<script module>
  // BrowseDetail stories. Reads the catalog store for collections + library
  // items (category/collection resolution). preset 'home' seeds both.
  import { defineMeta } from '@storybook/addon-svelte-csf';
  import BrowseDetail from './BrowseDetail.svelte';

  const { Story } = defineMeta({
    title: 'Components/BrowseDetail',
    component: BrowseDetail,
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

<Story name="Collection" args={{ preset: 'home', kind: 'collection', key: 'mock-collection' }}>
  <BrowseDetail kind="collection" key="mock-collection" />
</Story>

<Story name="Category" args={{ preset: 'home', kind: 'category', key: 'Action' }}>
  <BrowseDetail kind="category" key="Action" />
</Story>

<Story name="Playlist" args={{ preset: 'home', kind: 'playlist', key: 'Playlists' }}>
  <BrowseDetail kind="playlist" key="Playlists" />
</Story>
