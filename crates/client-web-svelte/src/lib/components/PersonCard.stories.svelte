<script module lang="ts">
  // PersonCard stories — a single cast/person thumbnail card (built on
  // CardSurface). Extracted from SectionPeople for independent testing.
  import { defineMeta } from '@storybook/addon-svelte-csf';
  import PersonCard from './PersonCard.svelte';
  import type { ItemMetadataPerson } from '$lib/api';

  const { Story } = defineMeta({
    title: 'Components/PersonCard',
    tags: ['autodocs'],
    args: { preset: 'empty' },
    parameters: {
      docs: {
        description: {
            component: 'A single cast/person card: 2/3 tile with photo or initials placeholder + name + optional character/role subtitle. Built on CardSurface. Clicking calls `onnavigate(personId)`.',
        },
      },
    },
  });

  const withPhoto: ItemMetadataPerson = {
    id: 1,
    person_id: 501,
    name: 'Ada Lovelace',
    character_name: 'The Architect',
    cached_image_path: '/people/501.jpg',
    sort_order: 0,
  };
  const noPhoto: ItemMetadataPerson = {
    id: 2,
    person_id: 502,
    name: 'Alan Turing',
    character_name: 'The Cryptographer',
    sort_order: 1,
  };
  const crew: ItemMetadataPerson = {
    id: 3,
    person_id: 504,
    name: 'Claude Shannon',
    role: 'Director',
    department: 'Directing',
    sort_order: 3,
  };
  const noop = (_id: number) => {};
</script>

<Story name="With Photo" args={{ preset: 'empty' }} asChild>
  <PersonCard person={withPhoto} onnavigate={noop} />
</Story>

<Story name="No Photo (Initials)" args={{ preset: 'empty' }} asChild>
  <PersonCard person={noPhoto} onnavigate={noop} />
</Story>

<Story name="Crew (Role)" args={{ preset: 'empty' }} asChild>
  <PersonCard person={crew} onnavigate={noop} />
</Story>
