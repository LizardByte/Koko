<script lang="ts">
  // Home page — replaces renderHomePage() (../client-web/src/app/homeView.ts).
  // Demonstrates: data load on mount, $derived for the active library's home,
  // and the Shelf + MediaCard components doing the rendering work that the
  // vanilla client re-stringifies on every state change.
  import { onMount } from 'svelte';
  import { getHome, getLibraries, type MediaHome, type MediaLibrary } from '$lib/api';
  import Shelf from '$lib/components/Shelf.svelte';
  import Spinner from '$lib/components/Spinner.svelte';

  let home = $state<MediaHome | undefined>(undefined);
  let libraries = $state<MediaLibrary[]>([]);
  let activeLibraryId = $state<number | undefined>(undefined);
  let loading = $state(true);
  let error = $state<string | undefined>(undefined);

  const shelves = $derived(home?.shelves ?? []);

  async function load() {
    loading = true;
    error = undefined;
    try {
      [home, libraries] = await Promise.all([getHome(activeLibraryId), getLibraries()]);
    } catch (err) {
      error = err instanceof Error ? err.message : String(err);
    } finally {
      loading = false;
    }
  }

  function selectLibrary(id: number | undefined) {
    activeLibraryId = id;
    load();
  }

  onMount(load);
</script>

<svelte:head><title>Koko — Home</title></svelte:head>

{#if loading && !home}
  <Spinner label="Loading your library…" />
{:else if error}
  <div class="empty-state">Failed to load home: {error}</div>
{:else if shelves.length === 0}
  <div class="empty-state">No content yet. Add a library in Settings.</div>
{:else}
  {#if libraries.length > 1}
    <div class="library-tabs">
      <button
        class:active={activeLibraryId === undefined}
        onclick={() => selectLibrary(undefined)}
      >
        All
      </button>
      {#each libraries as lib (lib.id)}
        <button
          class:active={activeLibraryId === lib.id}
          onclick={() => selectLibrary(lib.id)}
        >
          {lib.name}
        </button>
      {/each}
    </div>
  {/if}

  {#each shelves as shelf (shelf.id)}
    {#if shelf.items.length > 0}
      <Shelf title={shelf.title} items={shelf.items} id={shelf.id} />
    {/if}
  {/each}

  {#if home?.collections.length}
    <section class="collections">
      <h2>Collections</h2>
      <div class="collection-grid">
        {#each home.collections as collection (collection.id)}
          <div class="collection-card">
            <div class="collection-name">{collection.name}</div>
            {#if collection.overview}
              <div class="collection-overview muted">{collection.overview}</div>
            {/if}
            <div class="collection-count muted">{collection.item_count} items</div>
          </div>
        {/each}
      </div>
    </section>
  {/if}
{/if}

<style>
  .library-tabs {
    display: flex;
    gap: 0.5rem;
    margin-bottom: 1.5rem;
    flex-wrap: wrap;
  }
  .library-tabs button {
    padding: 0.35rem 0.8rem;
    border-radius: 999px;
    border: 1px solid var(--koko-border, #ddd);
    background: var(--koko-surface, #fff);
    cursor: pointer;
    font-size: 0.85rem;
  }
  .library-tabs button.active {
    background: #2563eb;
    color: #fff;
    border-color: #2563eb;
  }
  .collections {
    margin-top: 1rem;
  }
  .collections h2 {
    font-size: 1.05rem;
    font-weight: 600;
    margin: 0 0 0.6rem;
  }
  .collection-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(220px, 1fr));
    gap: 0.8rem;
  }
  .collection-card {
    border: 1px solid var(--koko-border, #ddd);
    border-radius: 8px;
    padding: 0.9rem;
    background: var(--koko-surface, #fff);
  }
  .collection-name {
    font-weight: 600;
    margin-bottom: 0.3rem;
  }
  .collection-overview {
    font-size: 0.82rem;
    margin-bottom: 0.4rem;
  }
  .collection-count {
    font-size: 0.78rem;
  }
  .muted {
    color: var(--koko-muted, #777);
  }
  .empty-state {
    color: var(--koko-muted, #777);
    padding: 2rem;
    text-align: center;
  }
</style>
