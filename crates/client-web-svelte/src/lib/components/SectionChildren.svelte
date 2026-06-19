<script lang="ts">
  // SectionChildren — replaces renderSelectedItemChildrenSection()
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
    <div class="hierarchy-item-grid item-grid" class:season-episodes-grid={isSeason}>
      {#each item.children as child (child.id)}
        <MediaCard item={child} />
      {/each}
    </div>
  </section>
{/if}

<style>
  /*
   * Component-owned (SectionChildren-only). The base .item-grid is shared
   * (app.css); the season-episodes variant narrows the columns for episode
   * thumbnails. Values mirror vanilla style.css:1601-1616.
   */
  .hierarchy-item-grid {
    margin-top: 0.85rem;
  }

  .hierarchy-item-grid.season-episodes-grid {
    grid-template-columns: repeat(auto-fill, minmax(240px, 1fr));
    gap: 1.1rem;
  }

  .hierarchy-item-grid.season-episodes-grid :global(.episode-card) {
    gap: 0.55rem;
  }

  .hierarchy-item-grid.season-episodes-grid :global(.media-card-art.episode) {
    padding: 1rem;
  }
</style>
