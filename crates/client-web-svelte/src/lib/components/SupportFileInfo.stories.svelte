<script module lang="ts">
  // SupportFileInfo stories — Panel A of SectionSupport: "File and library"
  // info (library name, folders, source path, last-modified).
  import { defineMeta } from '@storybook/addon-svelte-csf';
  import SupportFileInfo from './SupportFileInfo.svelte';
  import { movieDetail } from '$lib/storybook/fixtures';
  import { mockLibraries } from '$lib/storybook/fixtures';
  import type { MediaLibrary } from '$lib/api';

  const { Story } = defineMeta({
    title: 'Components/SupportFileInfo',
    tags: ['autodocs'],
    args: { preset: 'home' },
    parameters: {
      docs: {
        description: {
            component: 'Panel A of SectionSupport: library name, folder count, source path, last-modified time. Props: item, library? (falls back to "Unknown").',
        },
      },
    },
  });

  const item = movieDetail();
  const library: MediaLibrary | undefined = mockLibraries()[0];
</script>

<Story name="With Library" args={{ preset: 'home' }} asChild>
  <SupportFileInfo {item} {library} />
</Story>

<Story name="No Library" args={{ preset: 'home' }} asChild>
  <SupportFileInfo {item} />
</Story>
