<script module>
  // HomeNavbar stories. Reads the catalog store (homeTab, searchQuery) +
  // libraries store (active library for scan/refresh buttons). preset 'home'
  // seeds a library-less home (no scan/refresh buttons); a per-library route
  // shows them.
  import { defineMeta } from '@storybook/addon-svelte-csf';
  import HomeNavbar from './HomeNavbar.svelte';

  const { Story } = defineMeta({
    title: 'Components/HomeNavbar',
    component: HomeNavbar,
    tags: ['autodocs'],
    args: { preset: 'home' },
    parameters: {
      docs: {
        description: {
          component:
            'Home top bar: 5 browse tabs, search form with toggle button (submit vs clear), and per-library scan/refresh-metadata buttons when a library is active.',
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
