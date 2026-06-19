<script lang="ts">
  // Home — Phase 1 placeholder. Phase 2 replaces this with the real home
  // (navbar, hero, shelves, tabs, search). Loads the catalog + libraries so
  // the rail + shell have real data and we can verify the foundation end to end.
  import { onMount } from 'svelte';
  import { catalog, libraries } from '$lib/stores';

  onMount(async () => {
    await Promise.all([catalog.loadHome(), libraries.load()]);
  });

  const shelves = $derived(catalog.home?.shelves ?? []);
</script>

<svelte:head><title>Koko — Home</title></svelte:head>

<section class="panel page-panel" style="padding:1.2rem">
  <h2 style="margin:0 0 0.5rem">Koko — Phase 1 (shell + auth)</h2>
  <p class="muted" style="margin:0 0 1rem">
    Sidebar rail, page backdrop, auth gating, and split stores are in place.
    Phase 2 will replace this placeholder with the real home (hero, shelves,
    tabs, search).
  </p>

  {#if shelves.length}
    <p class="muted">Loaded {shelves.length} shelves from the mock API:</p>
    <ul>
      {#each shelves as shelf (shelf.id)}
        <li><strong>{shelf.title}</strong> — {shelf.items.length} items</li>
      {/each}
    </ul>
  {/if}
</section>
