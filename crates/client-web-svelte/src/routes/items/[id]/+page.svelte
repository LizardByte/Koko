<script lang="ts">
  // Item detail page — replaces renderItemPage()/renderSelectedItemPage()
  // (../client-web/src/app/itemPersonView.ts:1035-1095). Composes breadcrumbs,
  // hero, people rail, extras rail, children section, collection rails, and
  // the support grid. Loads via the item store on mount + on id change.
  import { onMount } from 'svelte';
  import { page } from '$app/state';
  import { item, ui } from '$lib/stores';
  import ItemBreadcrumbs from '$lib/components/ItemBreadcrumbs.svelte';
  import ItemHero from '$lib/components/ItemHero.svelte';
  import ItemPeople from '$lib/components/ItemPeople.svelte';
  import ItemExtras from '$lib/components/ItemExtras.svelte';
  import ItemChildren from '$lib/components/ItemChildren.svelte';
  import ItemSupport from '$lib/components/ItemSupport.svelte';

  const itemId = $derived(Number(page.params.id));

  onMount(() => {
    if (Number.isFinite(itemId)) item.loadItem(itemId);
  });

  // Reload when navigating between items without unmounting.
  $effect(() => {
    if (Number.isFinite(itemId)) {
      item.loadItem(itemId).catch((err: unknown) => {
        ui.setError(err instanceof Error ? err.message : String(err));
      });
    }
  });
</script>

<svelte:head><title>{item.item ? `${item.item.display_title} — Koko` : 'Koko'}</title></svelte:head>

{#if item.loading && !item.item}
  <section class="panel page-panel">
    <div class="empty-state">Loading item details…</div>
  </section>
{:else if item.item}
  <section class="item-page">
    <ItemBreadcrumbs item={item.item} />
    <ItemHero itemValue={item.item} />
    <ItemPeople metadata={item.metadata} />
    <ItemExtras extras={item.item.extras} />
    <ItemChildren item={item.item} />
    <ItemSupport item={item.item} metadata={item.metadata} />
  </section>
{:else}
  <section class="panel page-panel">
    <div class="empty-state">Item not found.</div>
  </section>
{/if}

<style>
  .item-page {
    display: flex;
    flex-direction: column;
    gap: 1.25rem;
    padding-top: 1rem;
    padding-bottom: 1.2rem;
  }
</style>
