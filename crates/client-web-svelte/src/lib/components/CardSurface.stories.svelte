<script module lang="ts">
  // CardSurface stories — documents the shared card shell + its config knobs
  // (tileRadius, aspectRatio, bordered, hover). Uses CardSurfacePreview (a
  // story-only wrapper) so the addon controls update the card live; raw
  // CardSurface requires art/body snippets which can't be sourced from args.
  //
  // Leaf cards (PersonCard, MediaExtraCard) build on CardSurface; MediaCard
  // keeps its own markup.
  import { defineMeta } from '@storybook/addon-svelte-csf';
  import CardSurfacePreview from './CardSurfacePreview.svelte';

  const { Story } = defineMeta({
    title: 'Components/CardSurface',
    component: CardSurfacePreview,
    tags: ['autodocs'],
    args: {
      tileRadius: 12,
      aspectRatio: '2 / 3',
      bordered: true,
      hover: 'none',
    },
    argTypes: {
      tileRadius: { control: { type: 'number', min: 0, max: 32, step: 1 } },
      aspectRatio: {
        control: { type: 'select' },
        options: ['2 / 3', '16 / 9', '1 / 1', '3 / 4'],
      },
      bordered: { control: 'boolean' },
      hover: { control: { type: 'radio' }, options: ['none', 'tile'] },
    },
    parameters: {
      docs: {
        description: {
          component:
            'Shared card shell: a transparent <button> root + a rounded, overflow-clipped tile wrapper. Used by PersonCard + MediaExtraCard; MediaCard keeps its own (global CSS coupling). Card-wide bugs (corner bleed, hover leak) are fixed here in one place. Configure via tileRadius, aspectRatio, bordered, hover.',
        },
      },
    },
  });
</script>

<!-- Each story is just args — with `component: CardSurfacePreview` set, the
     args spread onto the preview component as props, making all controls
     live. Switch between presets via the story tabs; tweak via the Controls
     panel. -->

<Story
  name="Poster (2/3, bordered, hover)"
  args={{ tileRadius: 18, aspectRatio: '2 / 3', bordered: true, hover: 'tile' }}
/>

<Story
  name="Backdrop (16/9, bordered)"
  args={{ tileRadius: 8, aspectRatio: '16 / 9', bordered: true, hover: 'none' }}
/>

<Story
  name="Square Avatar (1/1, no border)"
  args={{ tileRadius: 12, aspectRatio: '1 / 1', bordered: false, hover: 'none' }}
/>
