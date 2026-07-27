<script module>
  // SectionExtras stories. Props-driven (extras). Playback click is a stub
  // (playbackController spike) — harmless in stories.
  import { defineMeta } from '@storybook/addon-svelte-csf';
  import SectionExtras from './SectionExtras.svelte';
  import { mockExtras } from '$lib/storybook/fixtures';

  const { Story } = defineMeta({
    title: 'Components/SectionExtras',
    component: SectionExtras,
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
  <SectionExtras {extras} />
</Story>

<Story name="Empty" args={{ preset: 'empty' }} asChild>
  <SectionExtras extras={[]} />
</Story>
