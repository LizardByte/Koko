<script module>
  // PersonCredits stories. Props-driven (credits). Empty-state for now; a
  // populated variant needs a realistic MetadataPersonItemCredit fixture
  // (follow-up when the person view gets deeper attention).
  import { defineMeta } from '@storybook/addon-svelte-csf';
  import PersonCredits from './PersonCredits.svelte';

  const { Story } = defineMeta({
    title: 'Components/PersonCredits',
    component: PersonCredits,
    tags: ['autodocs'],
    args: { preset: 'empty' },
    parameters: {
      docs: {
        description: {
          component:
            'Credit grid with hover/focus trays expanding seasons/episodes. Empty-state shown here; populated variant pending a realistic credit fixture.',
        },
      },
    },
  });
</script>

<Story name="Empty" args={{ preset: 'empty' }} asChild>
  <PersonCredits credits={[]} />
</Story>
