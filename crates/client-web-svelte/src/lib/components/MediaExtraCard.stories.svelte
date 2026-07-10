<script module lang="ts">
  // MediaExtraCard stories — a single trailer/theme thumbnail card (built on
  // CardSurface). Extracted from SectionExtras for independent testing.
  import { defineMeta } from '@storybook/addon-svelte-csf';
  import MediaExtraCard from './MediaExtraCard.svelte';
  import type { MediaItemExtra } from '$lib/api';

  const { Story } = defineMeta({
    title: 'Components/MediaExtraCard',
    tags: ['autodocs'],
    args: { preset: 'empty' },
    parameters: {
      docs: {
        description: {
            component: 'A single trailer/theme-song thumbnail card: 16/9 tile with image or gradient + placeholder icon, play overlay, title, and meta line. Built on CardSurface. Clicking calls `onplay(extra)`.',
        },
      },
    },
  });

  const trailer: MediaItemExtra = {
    extra_type: 'trailer',
    url: 'mock://trailer',
    title: 'Official Trailer',
    duration_seconds: 152,
  } as MediaItemExtra;
  const theme: MediaItemExtra = {
    extra_type: 'theme_song',
    url: 'mock://theme',
    title: 'Main Theme',
    duration_seconds: 88,
  } as MediaItemExtra;
  const noop = (_e: MediaItemExtra) => {};
</script>

<Story name="Trailer" args={{ preset: 'empty' }} asChild>
  <MediaExtraCard extra={trailer} onplay={noop} />
</Story>

<Story name="Theme Song" args={{ preset: 'empty' }} asChild>
  <MediaExtraCard extra={theme} onplay={noop} />
</Story>
