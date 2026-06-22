<script lang="ts">
  // Home content — shared by `/` and `/libraries/[id]`. Extracted so both routes
  // render identical UI (the vanilla client's renderHomePage handles both).
  import { onMount } from 'svelte';
  import { goto } from '$app/navigation';
  import HomeNavbar from '$lib/components/HomeNavbar.svelte';
  import HomeFeature from '$lib/components/HomeFeature.svelte';
  import Shelf from '$lib/components/Shelf.svelte';
  import MediaCard from '$lib/components/MediaCard.svelte';
  import Icon from '$lib/components/Icon.svelte';
  import { catalog, libraries, ui, activities } from '$lib/stores';
  import { getArtworkUrl, getPersonImageUrl, resolveApiUrl } from '$lib/api';
  import { humanizeItemType, formatChildCount } from '$lib/ui';
  import { browseDetailPath } from '$lib/paths';
  import { categorySummaries } from '$lib/selectors';
  import { navRegion, navigateShelfRow, navigateList, firstCardOfFirstShelf } from '$lib/actions/navRegion';

  type Props = { libraryId?: number };
  let { libraryId }: Props = $props();

  // Thumbnail resolvers for the four search-result types — mirrors vanilla
  // renderSearchResultRow (homeView.ts:526-590).
  function itemThumb(id: number, artworkUpdatedAt?: number): string {
    return getArtworkUrl(id, 'poster', artworkUpdatedAt);
  }
  function collectionThumb(artwork?: string | null, backdrop?: string | null): string | undefined {
    const url = artwork ?? backdrop;
    return url ? resolveApiUrl(url) : undefined;
  }
  function personThumb(person: { cached_image_path?: string | null; image_url?: string | null; id: number }): string | undefined {
    if (person.cached_image_path) return getPersonImageUrl(person.id);
    if (person.image_url) return resolveApiUrl(person.image_url);
    return undefined;
  }

  onMount(async () => {
    try {
      await Promise.all([catalog.loadHome(libraryId), libraries.load()]);
      ui.clearError();
    } catch (err) {
      ui.setError(err instanceof Error ? err.message : String(err));
    }
  });

  // Reload home when the library prop changes.
  $effect(() => {
    libraryId;
    catalog
      .loadHome(libraryId)
      .then(() => ui.clearError())
      .catch((err) => ui.setError(err instanceof Error ? err.message : String(err)));
  });

  // Re-fetch home data when a metadata-refresh or library-scan activity
  // updates (Phase 6.5d). Mirrors vanilla refreshPendingMetadataData's home-
  // page branch (app.ts:772-807). Activities are updated by the layout poll.
  $effect(() => {
    const acts = activities.systemActivities?.activities ?? [];
    const hasActive = acts.some(
      (a) => (a.category === 'metadata_refresh' || a.category === 'library_scan') && a.state !== 'completed' && a.state !== 'failed',
    );
    if (hasActive) {
      catalog.loadHome(libraryId).catch(() => {});
    }
  });

  const shelves = $derived((catalog.home?.shelves ?? []).filter((shelf) => shelf.items.length > 0));
  const collections = $derived(catalog.home?.collections ?? []);
  // Genre categories derived from the loaded library items (vanilla
  // categorySummaries — selectors.ts:135-163). Empty until libraryItems load.
  const categories = $derived(categorySummaries(catalog.libraryItems));
  const hasContent = $derived(shelves.length > 0 || collections.length > 0);

  const preview = $derived(resolvePreview());

  function resolvePreview():
    | { kind: 'collection'; collection: (typeof collections)[number] }
    | { kind: 'item'; item: NonNullable<(typeof shelves)[number]['items'][number]> }
    | undefined {
    if (catalog.searchQuery.trim() && catalog.searchResults.length) {
      const first = catalog.searchResults[0];
      if (first.result_type === 'item') return { kind: 'item', item: first.item };
      if (first.result_type === 'collection') return { kind: 'collection', collection: first.collection };
    }
    if (catalog.homeTab === 'collections' && collections.length) {
      const match = collections.find((collection) => collection.id === catalog.homePreviewCollectionId);
      return { kind: 'collection', collection: match ?? collections[0] };
    }
    const firstItem = shelves[0]?.items[0];
    if (firstItem) return { kind: 'item', item: firstItem };
    return undefined;
  }

  const libraryItems = $derived(catalog.libraryItems);
  $effect(() => {
    if (catalog.homeTab === 'library' && libraryItems.length === 0 && !catalog.libraryItemsLoading) {
      catalog.loadLibraryItems(libraryId).catch(() => {});
    }
  });

  let contentRegion = $state<HTMLElement | undefined>(undefined);

  // Navigate between shelves vertically (up/down).
  function navigateShelves(direction: 'up' | 'down', current: HTMLElement): boolean {
    const allShelves = Array.from(
      contentRegion?.querySelectorAll<HTMLElement>('.shelf') ?? [],
    ).filter((s) => s.offsetParent !== null);
    if (allShelves.length === 0) return false;

    const currentShelf = allShelves.find((s) => s.contains(current)) ?? allShelves[0];
    const idx = allShelves.indexOf(currentShelf);

    if (direction === 'down' && idx < allShelves.length - 1) {
      const card = allShelves[idx + 1].querySelector<HTMLElement>('.media-card:not(:disabled)');
      card?.focus();
      card?.scrollIntoView({ block: 'nearest', inline: 'nearest' });
      return true;
    }
    if (direction === 'up' && idx > 0) {
      const cards = allShelves[idx - 1].querySelectorAll<HTMLElement>('.media-card:not(:disabled)');
      const card = cards[cards.length - 1];
      card?.focus();
      card?.scrollIntoView({ block: 'nearest', inline: 'nearest' });
      return true;
    }
    return false;
  }
</script>

<svelte:head><title>Koko — Home</title></svelte:head>

<HomeNavbar />

<div class="main-shell-inner home-content" bind:this={contentRegion} use:navRegion={{
  name: 'content',
  navigate: (direction, current) => {
    // Left/right: navigate within a shelf row (+ scroll).
    if (direction === 'left' || direction === 'right') {
      const row = current.closest('[data-shelf-row], .shelf-row') as HTMLElement | null;
      if (row) {
        return navigateShelfRow(direction, current, row);
      }
      // Not in a shelf (e.g., grid view) — use simple list nav.
      return navigateList(direction, current, current.closest('.item-grid, .category-grid') ?? contentRegion ?? current.parentElement!, { horizontal: true });
    }
    // Up/down: navigate between shelves.
    if (direction === 'up' || direction === 'down') {
      return navigateShelves(direction, current);
    }
    return false;
  },
  enter: {
    // Entering content from sidebar (left) or navbar (down) → first card of first shelf.
    left: () => firstCardOfFirstShelf(contentRegion!),
    right: () => firstCardOfFirstShelf(contentRegion!),
    down: () => firstCardOfFirstShelf(contentRegion!),
  },
}}>
  {#if preview}
    <HomeFeature
      collection={preview.kind === 'collection' ? preview.collection : undefined}
      item={preview.kind === 'item' ? preview.item : undefined}
    />
  {/if}

  {#if catalog.searchQuery.trim() && catalog.searchResults.length}
    <section class="panel page-panel search-results-section">
      <div class="shelf-header">
        <h3>Search results</h3>
        <span>{catalog.searchResults.length} matches</span>
      </div>
      <div class="search-results-list">
        {#each catalog.searchResults as result, i (i)}
          {#if result.result_type === 'item'}
            <button type="button" class="search-result-row" onclick={() => goto(`/items/${result.item.id}`)}>
              <span class="search-result-thumb" style={`background-image: url('${itemThumb(result.item.id, result.item.artwork_updated_at)}');`}></span>
              <span class="search-result-copy">
                <strong>{result.item.display_title}</strong>
                <span>{[libraries.byId(result.item.library_id)?.name ?? 'Library', humanizeItemType(result.item.item_type), formatChildCount(result.item)].filter(Boolean).join(' · ')}</span>
                {#if result.item.overview}<small>{result.item.overview}</small>{/if}
              </span>
            </button>
          {:else if result.result_type === 'collection'}
            <button type="button" class="search-result-row" onclick={() => goto(browseDetailPath('collection', result.collection.id, libraryId))}>
              <span class="search-result-thumb" style={collectionThumb(result.collection.artwork_url, result.collection.backdrop_url) ? `background-image: url('${collectionThumb(result.collection.artwork_url, result.collection.backdrop_url)}');` : ''}>
                {#if !collectionThumb(result.collection.artwork_url, result.collection.backdrop_url)}<Icon name="image" size={20} />{/if}
              </span>
              <span class="search-result-copy">
                <strong>{result.collection.name}</strong>
                <span>Collection · {result.collection.item_count} title{result.collection.item_count === 1 ? '' : 's'}</span>
                {#if result.collection.overview}<small>{result.collection.overview}</small>{/if}
              </span>
            </button>
          {:else if result.result_type === 'person'}
            <button type="button" class="search-result-row" onclick={() => goto(`/people/${result.person.id}`)}>
              <span class="search-result-thumb" style={personThumb(result.person) ? `background-image: url('${personThumb(result.person)}');` : ''}>
                {#if !personThumb(result.person)}<Icon name="user-plus" size={20} />{/if}
              </span>
              <span class="search-result-copy">
                <strong>{result.person.name}</strong>
                <span>{result.person.known_for.slice(0, 3).join(' · ') || 'Person'}</span>
                {#if result.person.biography}<small>{result.person.biography}</small>{/if}
              </span>
            </button>
          {:else if result.result_type === 'playlist'}
            <button type="button" class="search-result-row" onclick={() => goto(`/items/playlists/${encodeURIComponent(result.playlist.id)}`)}>
              <span class="search-result-thumb"><Icon name="music" size={20} /></span>
              <span class="search-result-copy">
                <strong>{result.playlist.name}</strong>
                <span>Playlist · {result.playlist.item_count} title{result.playlist.item_count === 1 ? '' : 's'}</span>
                {#if result.playlist.overview}<small>{result.playlist.overview}</small>{/if}
              </span>
            </button>
          {/if}
        {/each}
      </div>
    </section>
  {:else if !hasContent}
    <section class="shelf">
      <div class="empty-state">No shelves are available yet. Add a library to get started.</div>
    </section>
  {:else}
    {#if catalog.homeTab === 'recommended'}
      <section class="shelf-stack panel page-panel">
        {#each shelves as shelf (shelf.id)}
          <Shelf title={shelf.title} items={shelf.items} id={shelf.id} rowCountId={shelf.id} />
        {/each}
      </section>
    {:else if catalog.homeTab === 'library'}
      <section class="panel page-panel home-tab-panel">
        <div class="shelf-header">
          <h3>{libraryId ? libraries.byId(libraryId)?.name ?? 'Library' : 'All libraries'}</h3>
        </div>
        {#if catalog.libraryItemsLoading}
          <div class="empty-state tight">Loading items…</div>
        {:else if libraryItems.length === 0}
          <div class="empty-state tight">No items found.</div>
        {:else}
          <div class="item-grid">
            {#each libraryItems as libraryItem (libraryItem.id)}
              <MediaCard item={libraryItem} />
            {/each}
          </div>
        {/if}
      </section>
    {:else if catalog.homeTab === 'collections'}
      <section class="panel page-panel home-tab-panel">
        <div class="shelf-header">
          <h3>Collections</h3>
          <span>{collections.length} collection{collections.length === 1 ? '' : 's'}</span>
        </div>
        {#if collections.length === 0}
          <div class="empty-state tight">No collections are linked yet.</div>
        {:else}
          <div class="item-grid">
            {#each collections as collection (collection.id)}
              <button type="button" class="media-card collection-browse-card" onclick={() => goto(browseDetailPath('collection', collection.id, libraryId))}>
                <span class="media-card-art collection" style={collection.artwork_url ? `background-image: url('${collection.artwork_url}');` : ''}>
                  <span class="media-card-kind-row">
                    <span class="media-card-kind"><Icon name="layers" size={16} /></span>
                  </span>
                </span>
                <span class="media-card-title">{collection.name}</span>
                <span class="media-card-meta">{collection.item_count} title{collection.item_count === 1 ? '' : 's'}</span>
              </button>
            {/each}
          </div>
        {/if}
      </section>
    {:else if catalog.homeTab === 'playlists'}
      <section class="panel page-panel home-tab-panel">
        <div class="category-grid">
          <button
            type="button"
            class="category-card panel filter-card-button"
            onclick={() => goto(browseDetailPath('playlist', 'Playlists', libraryId))}
          >
            <div class="category-card-header">
              <strong>Playlists</strong>
              <span class="tag">0 titles</span>
            </div>
            <p class="muted">Playlist creation is planned. Items will appear here when playlists are available.</p>
          </button>
        </div>
      </section>
    {:else if catalog.homeTab === 'categories'}
      <section class="panel page-panel home-tab-panel">
        <div class="shelf-header"><h3>Categories</h3></div>
        {#if categories.length === 0}
          {#if catalog.libraryItemsLoading}
            <div class="empty-state tight">Loading library items…</div>
          {:else}
            <div class="empty-state tight">No genre metadata is available yet for the current library.</div>
          {/if}
        {:else}
          <div class="category-grid">
            {#each categories as category (category.genre)}
              <button
                type="button"
                class="category-card panel filter-card-button"
                onclick={() => goto(browseDetailPath('category', category.genre, libraryId))}
              >
                <div class="category-card-header">
                  <strong>{category.genre}</strong>
                  <span class="tag">{category.count} title{category.count === 1 ? '' : 's'}</span>
                </div>
                <p class="muted">{category.items.slice(0, 3).map((item) => item.display_title).join(' · ') || 'No titles yet'}</p>
              </button>
            {/each}
          </div>
        {/if}
      </section>
    {/if}
  {/if}
</div>

<style>
  /*
   * Component-owned (HomeContent-only). Values mirror vanilla style.css
   * :1257-1337. .item-grid / .shelf-header / .shelf / .tag are shared
   * (app.css). .home-tab-panel / .search-results-section / collection cards
   * are HomeContent-only.
   */
  .home-content {
    gap: 0;
  }

  .home-tab-panel {
    padding: 1.2rem;
  }

  .search-results-section {
    padding: 1.2rem;
  }

  .search-results-section .shelf-header {
    margin-bottom: 0.8rem;
  }

  .search-results-list {
    display: flex;
    flex-direction: column;
    gap: 0.8rem;
  }

  .search-result-row {
    display: grid;
    grid-template-columns: 64px minmax(0, 1fr);
    gap: 0.85rem;
    align-items: center;
    padding: 0.65rem;
    text-align: left;
    border-radius: 12px;
    background: rgba(255, 255, 255, 0.04);
    box-shadow: none;
    color: inherit;
  }

  .search-result-row:hover,
  .search-result-row:focus-visible {
    background: rgba(255, 255, 255, 0.08);
  }

  .search-result-thumb {
    width: 64px;
    aspect-ratio: 2 / 3;
    border-radius: 8px;
    background: rgba(255, 255, 255, 0.08) center / cover no-repeat;
  }

  .search-result-copy {
    display: flex;
    flex-direction: column;
    gap: 0.2rem;
    min-width: 0;
  }

  .search-result-copy span,
  .search-result-copy small {
    color: #9ab1d1;
  }

  .search-result-copy small {
    display: -webkit-box;
    overflow: hidden;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
  }

  .collection-browse-card {
    cursor: pointer;
  }

  .collection-browse-card .media-card-art.collection {
    background: linear-gradient(180deg, rgba(93, 123, 255, 0.6), rgba(22, 31, 54, 0.92));
    background-size: cover;
    background-position: center;
  }

  /* Category cards (Collections/Categories/Playlists tabs). Vanilla style.css
     :801-832. .panel/.tag/.muted are shared (app.css). */
  .category-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(260px, 1fr));
    gap: 0.9rem;
  }

  .filter-card-button {
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
    padding: 1rem 1.1rem;
    text-align: left;
    cursor: pointer;
  }

  .category-card-header {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    gap: 0.75rem;
  }

  .filter-card-button:hover {
    border-color: rgba(93, 123, 255, 0.35);
    background: rgba(255, 255, 255, 0.06);
  }
</style>
