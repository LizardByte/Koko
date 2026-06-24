<script lang="ts">
  // Story-only wrapper for the Icon "Customize" story. Takes color/alpha/size
  // as props (from Storybook args) and recolors every gallery icon via
  // currentColor. Lives alongside the story so the Icon component itself stays
  // minimal — production code never needs color/alpha props.
  import Icon, { ICONS } from './Icon.svelte';

  let {
    color = '#8dd35f',
    alpha = 1,
    size = 40,
  }: {
    color?: string;
    alpha?: number;
    size?: number;
    // Storybook decorator-only args (preset/route) are spread onto every
    // args-driven component; accept and ignore them so the Customize story's
    // args type-check cleanly. See withStores.ts decorator.
    preset?: unknown;
    route?: unknown;
  } = $props();

  const names = Object.keys(ICONS).toSorted();
</script>

<div class="icon-grid" style="color: {color}; opacity: {alpha}">
  {#each names as name (name)}
    <figure>
      <Icon name={name} {size} />
      <figcaption>{name}</figcaption>
    </figure>
  {/each}
</div>

<style>
  .icon-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(120px, 1fr));
    gap: 1rem;
    padding: 1rem;
    color: #f4f7fb;
  }
  figure {
    margin: 0;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 0.5rem;
    padding: 1rem 0.5rem;
    border-radius: 12px;
    background: rgba(255, 255, 255, 0.04);
  }
  figcaption {
    font-size: 0.78rem;
    color: #9ab1d1;
    font-family: monospace;
  }
</style>
