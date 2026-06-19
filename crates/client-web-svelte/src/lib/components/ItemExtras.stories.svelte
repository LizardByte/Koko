<script module>
  // ItemExtras stories. Props-driven (extras). Playback click is a stub
  // (playbackController spike) — harmless in stories.
  import { defineMeta } from '@storybook/addon-svelte-csf';
  import ItemExtras from './ItemExtras.svelte';
  import { mockExtras } from '$lib/storybook/fixtures';

  const { Story } = defineMeta({
    title: 'Components/ItemExtras',
    component: ItemExtras,
    tags: ['autodocs'],
    args: { preset: 'empty' },
    parameters: {
      docs: {
        description: {
          component: 'Trailer/theme-song thumbnails rail. Clicking dispatches a playback stub (not yet implemented). Empty-state when no extras.',
        },
      },
    },
  });

  const extras = mockExtras();
</script>

<Story name="Populated" args={{ preset: 'empty' }} asChild>
  <ItemExtras {extras} />
</Story>

<Story name="Empty" args={{ preset: 'empty' }} asChild>
  <ItemExtras extras={[]} />
</Story>
