<script module lang="ts">
  // PersonHero stories — the top summary banner of the person-detail page:
  // profile photo/initials, name, biography, known-for tags, age/birth info.
  // Fully props-driven (person + onBack callback).
  import { defineMeta } from '@storybook/addon-svelte-csf';
  import PersonHero from './PersonHero.svelte';
  import { mockPerson } from '$lib/storybook/fixtures';

  const { Story } = defineMeta({
    title: 'Components/PersonHero',
    tags: ['autodocs'],
    args: { preset: 'empty' },
    parameters: {
      docs: {
        description: {
          component:
            'Top summary banner for the person-detail page (analogous to SectionHero for items): profile photo or initials, name, biography, known-for tags, birth/death dates. Props: person (MetadataPersonResponse), onBack.',
        },
      },
    },
  });

  const person = mockPerson();
  const personWithImage = mockPerson({
    person: { ...mockPerson().person, cached_image_path: '/people/501.jpg' },
  });
  const minimalPerson = mockPerson({
    person: {
      id: 505,
      provider_id: 'tmdb',
      locale_key: 'en',
      name: 'Unknown Artist',
      known_for: [],
    },
    credits: [],
  });
  const noop = () => {};
</script>

<Story name="Full Profile" args={{ preset: 'empty' }} asChild>
  <PersonHero person={person} onBack={noop} />
</Story>

<Story name="With Image" args={{ preset: 'empty' }} asChild>
  <PersonHero person={personWithImage} onBack={noop} />
</Story>

<Story name="Minimal (No Bio)" args={{ preset: 'empty' }} asChild>
  <PersonHero person={minimalPerson} onBack={noop} />
</Story>
