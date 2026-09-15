<script module lang="ts">
  // SupportMetadata stories — Panel B of SectionSupport: "Link metadata" /
  // "Metadata" summary (provider tags, overview, attribution) or empty-state.
  import { defineMeta } from '@storybook/addon-svelte-csf';
  import SupportMetadata from './SupportMetadata.svelte';
  import { movieDetail, mockMetadata } from '$lib/storybook/fixtures';

  const { Story } = defineMeta({
    title: 'Components/SupportMetadata',
    tags: ['autodocs'],
    args: { preset: 'home' },
    parameters: {
      docs: {
        description: {
            component: 'Panel B of SectionSupport: primary provider match summary (tags, overview, attribution link) or empty-state when nothing is linked. Derives primaryMatch + provider internally. Props: item, metadata.',
        },
      },
    },
  });

  const item = movieDetail();
  const metadata = mockMetadata();
</script>

<Story name="Linked Metadata" args={{ preset: 'home' }} asChild>
  <SupportMetadata {item} {metadata} />
</Story>

<Story name="No Metadata (Empty)" args={{ preset: 'home' }} asChild>
  <SupportMetadata {item} metadata={undefined} />
</Story>
