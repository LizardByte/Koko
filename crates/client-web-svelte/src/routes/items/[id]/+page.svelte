<script lang="ts">
  // Item detail page — replaces renderItemPage()/renderSelectedItemPage()
  // (../client-web/src/app/itemPersonView.ts:1035-1095). Composes breadcrumbs,
  // hero, people rail, extras rail, children section, collection rails, and
  // the support grid. Loads via the item store on mount + on id change.
  import { page } from '$app/state';
  import { item, ui } from '$lib/stores';
  import SectionBreadcrumbs from '$lib/components/SectionBreadcrumbs.svelte';
  import SectionHero from '$lib/components/SectionHero.svelte';
  import SectionPeople from '$lib/components/SectionPeople.svelte';
  import SectionExtras from '$lib/components/SectionExtras.svelte';
  import SectionChildren from '$lib/components/SectionChildren.svelte';
  import SectionSupport from '$lib/components/SectionSupport.svelte';

  const itemId = $derived(Number(page.params.id));

  // Load on mount + reload when navigating between items without unmounting.
  // Clear any previous error on success (vanilla clears state.error in every
  // success branch — app.ts:340 — so navigating away from a failed item
  // removes the banner). Without this the error persists indefinitely.
  $effect(() => {
    if (Number.isFinite(itemId)) {
      item
        .loadItem(itemId)
        .then(() => ui.clearError())
        .catch((err: unknown) => {
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
    <SectionBreadcrumbs item={item.item} />
    <SectionHero itemValue={item.item} />
    <SectionPeople metadata={item.metadata} />
    <SectionExtras extras={item.item.extras} />
    <SectionChildren item={item.item} />
    <SectionSupport item={item.item} metadata={item.metadata} />
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
