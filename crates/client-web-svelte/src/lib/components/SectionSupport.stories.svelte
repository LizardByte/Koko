<script module>
  // SectionSupport stories. Props-driven (item + metadata). The item drives the
  // File/library panel; metadata drives the linked-metadata summary.
  import { defineMeta } from '@storybook/addon-svelte-csf';
  import SectionSupport from './SectionSupport.svelte';
  import { movieDetail, mockMetadata } from '$lib/storybook/fixtures';

  const { Story } = defineMeta({
    title: 'Components/SectionSupport',
    component: SectionSupport,
    tags: ['autodocs'],
    args: { preset: 'home' },
    parameters: {
      docs: {
        description: {
          component: 'Two-panel support grid: File & library info + linked-metadata summary (provider tags, overview, attribution).',
        },
      },
    },
  });

  const item = movieDetail();
  const metadata = mockMetadata();
</script>

<Story name="Linked" args={{ preset: 'home' }} asChild>
  <SectionSupport {item} {metadata} />
</Story>

<Story name="Unlinked" args={{ preset: 'home' }} asChild>
  <SectionSupport {item} metadata={undefined} />
</Story>
