<script lang="ts">
  // ItemBreadcrumbs — replaces renderSelectedItemBreadcrumbs()
  // (../client-web/src/app/itemPersonView.ts:736-750).
  //
  // Navigation is injectable via `onnavigate` so Storybook/tests can avoid
  // mocking $app/navigation; production leaves it unset and it falls back to
  // the real SvelteKit goto().
  import { goto } from '$app/navigation';
  import type { MediaItemDetail } from '$lib/api';

  type Props = { item: MediaItemDetail; onnavigate?: (itemId: number) => void };
  let { item, onnavigate = (id: number) => goto(`/items/${id}`) }: Props = $props();
</script>

{#if item.hierarchy.length}
  <nav class="item-breadcrumbs panel page-panel" aria-label="Item hierarchy">
    {#each item.hierarchy as ancestor, i (ancestor.id)}
      {#if i > 0}<span class="breadcrumb-separator">/</span>{/if}
      <button type="button" class="breadcrumb-button" onclick={() => onnavigate(ancestor.id)}>
        {ancestor.display_title}
      </button>
    {/each}
    <span class="breadcrumb-separator">/</span>
    <span class="breadcrumb-current">{item.display_title}</span>
  </nav>
{/if}

<style>
  .item-breadcrumbs {
    display: flex;
    gap: 0.45rem;
    flex-wrap: wrap;
    align-items: center;
    padding: 0.85rem 1rem;
  }
  .breadcrumb-button {
    padding: 0;
    background: transparent;
    box-shadow: none;
    color: #b7cae6;
  }
  .breadcrumb-button:hover {
    color: #fff;
  }
  .breadcrumb-separator,
  .breadcrumb-current {
    color: #86a0c7;
  }
</style>
