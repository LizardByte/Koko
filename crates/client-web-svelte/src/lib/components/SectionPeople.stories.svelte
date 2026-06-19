<script module>
  // ItemPeople stories. Reads the `metadata` prop; people come from the first
  // metadata match's `people` array (populated in mockMetadata). The component
  // renders nothing when there are zero people (no empty-state in vanilla),
  // so the "No People" story is intentionally blank — that's the faithful
  // behavior, not a bug.
  import { defineMeta } from '@storybook/addon-svelte-csf';
  import SectionPeople from './SectionPeople.svelte';
  import { mockMetadata } from '$lib/storybook/fixtures';

  const { Story } = defineMeta({
    title: 'Components/SectionPeople',
    component: SectionPeople,
    tags: ['autodocs'],
    args: { preset: 'home' },
    parameters: {
      docs: {
        description: {
          component:
            'Cast/people rail from the first metadata match. Renders nothing when the people array is empty (vanilla has no empty-state) — see the "No People" story.',
        },
      },
    },
  });

  const metadata = mockMetadata();
</script>

<Story name="With Cast" args={{ preset: 'home' }} asChild>
  <SectionPeople {metadata} />
</Story>

<Story name="No People" args={{ preset: 'home' }} asChild>
  <SectionPeople metadata={undefined} />
</Story>
