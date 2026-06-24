<script lang="ts">
  // Person detail page — replaces renderPersonPage()
  // (../client-web/src/app/itemPersonView.ts:526-581). Hero + credit grid.
  import { page } from '$app/state';
  import { goto } from '$app/navigation';
  import { item, ui } from '$lib/stores';
  import PersonHero from '$lib/components/PersonHero.svelte';
  import PersonCredits from '$lib/components/PersonCredits.svelte';

  const personId = $derived(Number(page.params.id));

  $effect(() => {
    if (Number.isFinite(personId)) {
      item
        .loadPerson(personId)
        .then(() => ui.clearError())
        .catch((err: unknown) => {
          ui.setError(err instanceof Error ? err.message : String(err));
        });
    }
  });

  function back() {
    // Vanilla eventBindings.ts:728-732 uses history.back() from the person
    // page (the #back-to-library handler), not a hard navigate to '/'.
    if (typeof window !== 'undefined' && window.history.length > 1) {
      window.history.back();
    } else {
      goto('/');
    }
  }
</script>

<svelte:head><title>{item.person ? `${item.person.person.name} — Koko` : 'Koko'}</title></svelte:head>

{#if item.loading && !item.person}
  <section class="panel page-panel">
    <div class="empty-state">Loading person…</div>
  </section>
{:else if item.person}
  <section class="item-page person-page">
    <PersonHero person={item.person} onBack={back} />
    <PersonCredits credits={item.person.credits} />
  </section>
{:else}
  <section class="panel page-panel">
    <div class="empty-state">Person not found.</div>
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
