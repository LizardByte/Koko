<script module>
  // ItemPeople stories. Reads the `metadata` prop; people come from the first
  // metadata match's `people` array (empty in mockMetadata, so this renders
  // the empty state). Add people to the fixture to exercise populated cards.
  import { defineMeta } from '@storybook/addon-svelte-csf';
  import ItemPeople from './ItemPeople.svelte';
  import { mockMetadata } from '$lib/storybook/fixtures';

  const { Story } = defineMeta({
    title: 'Components/ItemPeople',
    component: ItemPeople,
    tags: ['autodocs'],
    args: { preset: 'home' },
    parameters: {
      docs: {
        description: {
          component: 'Cast/people rail from the first metadata match. Empty-state when no people are present.',
        },
      },
    },
  });

  const metadata = mockMetadata();
</script>

<Story name="With Metadata" args={{ preset: 'home' }} asChild>
  <ItemPeople {metadata} />
</Story>

<Story name="No Metadata" args={{ preset: 'home' }} asChild>
  <ItemPeople metadata={undefined} />
</Story>
