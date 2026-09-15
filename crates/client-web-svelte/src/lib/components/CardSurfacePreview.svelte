<script lang="ts">
  // Story-only wrapper for the CardSurface stories. Takes the controlled props
  // (tileRadius, aspectRatio, bordered, hover) from Storybook args and renders
  // a CardSurface with fixed demo art/body snippets. This lets the addon
  // controls update the card live (the raw CardSurface requires snippets,
  // which can't be sourced from args). Lives alongside the stories so the
  // CardSurface component itself stays minimal — production callers pass their
  // own art/body snippets directly.
  import CardSurface from './CardSurface.svelte';

  let {
    tileRadius = 12,
    aspectRatio = '2 / 3',
    bordered = true,
    hover = 'none',
  }: {
    tileRadius?: number;
    aspectRatio?: string;
    bordered?: boolean;
    hover?: 'none' | 'tile';
    // Storybook decorator-only args (preset) are spread onto every args-driven
    // component by the withStories decorator; accept and ignore via the type
    // so the stories' args type-check cleanly. Not destructured on purpose.
    preset?: unknown;
  } = $props();
</script>

<div style="width: 200px;">
  <CardSurface {tileRadius} {aspectRatio} {bordered} {hover} label="Example">
    {#snippet art()}
      <div
        style="width:100%;height:100%;background:linear-gradient(135deg,#6366f1,#8b5cf6);display:grid;place-items:center;color:#fff;font-weight:700;"
      >
        Preview
      </div>
    {/snippet}
    {#snippet body()}
      <span style="font-weight:700;">Title</span>
      <span style="color:var(--color-text-muted);font-size:0.8rem;">Subtitle</span>
    {/snippet}
  </CardSurface>
</div>
