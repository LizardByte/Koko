<script lang="ts">
  // ItemChildren — replaces renderSelectedItemChildrenSection()
  // (../client-web/src/app/itemPersonView.ts:941-957). Seasons for shows,
  // episodes for seasons, contained items otherwise.
  import MediaCard from './MediaCard.svelte';
  import { countLabel } from '$lib';
  import type { MediaItemDetail } from '$lib/api';

  type Props = { item: MediaItemDetail };
  let { item }: Props = $props();

  const title = $derived(
    item.item_type === 'show' ? 'Seasons' : item.item_type === 'season' ? 'Episodes' : 'Contained items',
  );
  const isSeason = $derived(item.item_type === 'season');
</script>

{#if item.children.length}
  <section class="panel page-panel item-section">
    <div class="section-heading section-heading-actions">
      <div><h3>{title}</h3></div>
      <span class="muted">{countLabel(item.children.length, 'item')}</span>
    </div>
    <div class="item-grid" class:season-episodes-grid={isSeason}>
      {#each item.children as child (child.id)}
        <MediaCard item={child} />
      {/each}
    </div>
  </section>
{/if}

<style>
  .item-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(184px, 1fr));
    gap: 0.9rem;
    align-items: start;
  }
</style>
