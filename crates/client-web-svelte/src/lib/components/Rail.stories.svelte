<script module>
  // Rail stories. Reads the libraries store + $app/state (page) for active
  // state + auth store for the user card. preset 'home' seeds libraries +
  // current user; the `route` arg simulates which nav is active.
  import { defineMeta } from '@storybook/addon-svelte-csf';
  import Rail from './Rail.svelte';

  const { Story } = defineMeta({
    title: 'Components/Rail',
    component: Rail,
    tags: ['autodocs'],
    args: { preset: 'home' },
    parameters: {
      docs: {
        description: {
          component:
            'Persistent sidebar: brand block, Home + per-library nav (with metadata-refresh rings), user card, Settings, Sign out. Collapses to 88px via the `collapsed` prop (item pages).',
        },
      },
    },
  });
</script>

<Story name="Home Active" args={{ preset: 'home', route: '/' }} asChild>
  <Rail />
</Story>

<Story name="Library Active" args={{ preset: 'home', route: '/libraries/2' }} asChild>
  <Rail />
</Story>

<Story name="Collapsed" args={{ preset: 'home', route: '/items/101' }} asChild>
  <Rail collapsed={true} />
</Story>
