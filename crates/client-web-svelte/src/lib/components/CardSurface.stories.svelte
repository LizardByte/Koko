<script module lang="ts">
  // CardSurface stories — documents the shared card shell + its config knobs
  // (tileRadius, aspectRatio, bordered, hover). Leaf cards (PersonCard,
  // MediaExtraCard) build on this; MediaCard keeps its own markup.
  //
  // Note: these are static visual examples. CardSurface requires `art`/`body`
  // snippets which can't be sourced from Storybook args, so the controlled
  // props (tileRadius/aspectRatio/bordered/hover) are hardcoded per story
  // rather than wired to addon controls. The argTypes are declared for
  // documentation (Props table) but intentionally have no live controls.
  import { defineMeta } from '@storybook/addon-svelte-csf';
  import CardSurface from './CardSurface.svelte';

  const { Story } = defineMeta({
    title: 'Components/CardSurface',
    tags: ['autodocs'],
    args: { preset: 'empty' },
    parameters: {
      docs: {
        description: {
          component: 'Shared card shell: a transparent <button> root + a rounded, overflow-clipped tile wrapper. Used by PersonCard + MediaExtraCard; MediaCard keeps its own (global CSS coupling). Card-wide bugs (corner bleed, hover leak) are fixed here in one place. Configure via tileRadius, aspectRatio, bordered, hover.',
        },
      },
    },
  });
</script>

<Story name="Poster (2/3, bordered)" args={{ preset: 'empty' }} asChild>
  <div style="width: 200px;">
    <CardSurface tileRadius={18} aspectRatio="2 / 3" bordered hover="tile" label="Example">
      {#snippet art()}
        <div style="width:100%;height:100%;background:linear-gradient(135deg,#6366f1,#8b5cf6);display:grid;place-items:center;color:#fff;font-weight:700;">Poster</div>
      {/snippet}
      {#snippet body()}
        <span style="font-weight:700;">Title</span>
        <span style="color:var(--color-text-muted);font-size:0.8rem;">Subtitle</span>
      {/snippet}
    </CardSurface>
  </div>
</Story>

<Story name="Backdrop (16/9)" args={{ preset: 'empty' }} asChild>
  <div style="width: 244px;">
    <CardSurface tileRadius={8} aspectRatio="16 / 9" bordered label="Example">
      {#snippet art()}
        <div style="width:100%;height:100%;background:linear-gradient(135deg,rgba(57,78,123,0.88),rgba(12,18,32,0.94));display:grid;place-items:center;color:#e7f0ff;">Backdrop</div>
      {/snippet}
      {#snippet body()}
        <span style="font-weight:700;">Title</span>
      {/snippet}
    </CardSurface>
  </div>
</Story>

<Story name="Square Avatar (1/1, no border)" args={{ preset: 'empty' }} asChild>
  <div style="width: 120px;">
    <CardSurface tileRadius={12} aspectRatio="1 / 1" label="Example">
      {#snippet art()}
        <div style="width:100%;height:100%;background:rgba(255,255,255,0.08);display:grid;place-items:center;color:#dfe9ff;font-size:2rem;font-weight:700;">A</div>
      {/snippet}
    </CardSurface>
  </div>
</Story>
