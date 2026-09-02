<script module>
  // HomeNavbar stories. Reads the catalog store (homeTab, searchQuery) +
  // libraries store (active library for scan/refresh buttons). preset 'home'
  // seeds a library-less home (no scan/refresh buttons); a per-library route
  // shows them.
  import { defineMeta } from '@storybook/addon-svelte-csf';
  import HomeNavbar from './HomeNavbar.svelte';

  const { Story } = defineMeta({
    title: 'Fragments/HomeNavbar',
    component: HomeNavbar,
    tags: ['autodocs'],
    args: { preset: 'home' },
    parameters: {
      docs: {
        description: {
          component:
            'Home top bar: 5 browse tabs, search form with toggle button (submit vs clear), and per-library scan/refresh-metadata buttons when a library is active.\n\n' +
            '> ⚠️ **Store-driven component.** HomeNavbar takes **no props** — it reads the ' +
            '`catalog` store (active tab, search query) and `libraries` store (active library), ' +
            'seeded via the `preset` / `route` args (see `.storybook/decorators/withStores.ts`). ' +
            'The controls panel is sparse for this reason: switch `route` between `/` and ' +
            '`/libraries/:id` to drive the nav state. **TODO:** consider per-tab stories that ' +
            'seed catalog.homeTab so each tab is self-describing.',
        },
      },
    },
  });
</script>

<Story name="All Libraries" args={{ preset: 'home', route: '/' }} asChild>
  <HomeNavbar />
</Story>

<Story name="Active Library" args={{ preset: 'home', route: '/libraries/2' }} asChild>
  <HomeNavbar />
</Story>
