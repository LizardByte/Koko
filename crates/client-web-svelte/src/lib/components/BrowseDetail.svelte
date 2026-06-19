<script lang="ts">
  // BrowseDetail — replaces renderBrowseDetailPage() + the collection/category/
  // playlist sub-renderers (../client-web/src/app/homeView.ts:112-295).
  // Reads kind/key/libraryId from the route (page.params via the route page)
  // and derives its data purely from the catalog store — no browseFilter
  // state (vanilla's is dead code; see BROWSE_FILTER_PROPOSAL.md).
  import Button from './Button.svelte';
  import MediaCard from './MediaCard.svelte';
  import { goto } from '$app/navigation';
  import { catalog } from '$lib/stores';
  import {
    categorySummaries,
    itemsForCollection,
  } from '$lib/selectors';
  import { pageBackdropUrlForCollection } from '$lib/selectors';
  import { homeBrowsePath, type BrowseDetailKind } from '$lib/paths';

  type Props = {
    kind: BrowseDetailKind;
    key: string;
    libraryId?: number;
  };
  let { kind, key, libraryId }: Props = $props();

  // Ensure library items are loaded for category/collection resolution.
  $effect(() => {
    if (catalog.libraryItems.length === 0 && !catalog.libraryItemsLoading) {
      catalog.loadLibraryItems(libraryId).catch(() => {});
    }
  });

  // Decode the route key once (URL-encoded collection id / genre / playlist).
  const decodedKey = $derived(decodeURIComponent(key));

  // Resolve the active filter from existing catalog data — mirrors
  // browseFilterForRoute (homeView.ts:69-110).
  const collection = $derived(
    kind === 'collection'
      ? (catalog.home?.collections ?? []).find((entry) => entry.id === decodedKey)
      : undefined,
  );
  const category = $derived(
    kind === 'category'
      ? categorySummaries(catalog.libraryItems).find((entry) => entry.genre === decodedKey)
      : undefined,
  );

  // Items to render in the grid.
  const items = $derived(
    kind === 'collection' && collection
      ? itemsForCollection(collection, catalog.libraryItems)
      : kind === 'category' && category
        ? category.items
        : [],
  );

  // Hero copy.
  const eyebrow = $derived(
    kind === 'collection' ? 'Collection' : kind === 'category' ? 'Genre' : 'Playlist',
  );
  const title = $derived(
    kind === 'collection'
      ? collection?.name ?? decodedKey
      : kind === 'category'
        ? decodedKey
        : decodedKey,
  );
  const overview = $derived(
    kind === 'collection'
      ? collection?.overview ?? `${items.length} title${items.length === 1 ? '' : 's'} in this collection.`
      : kind === 'category'
        ? category?.items.slice(0, 5).map((item) => item.display_title).join(' · ') ||
          'No titles are currently linked to this genre.'
        : 'No playlist items are available yet.',
  );
  const heroBackdropUrl = $derived(
    kind === 'collection' ? pageBackdropUrlForCollection(collection) : undefined,
  );

  // Back target — the home browse path for the active library (or all).
  function back() {
    goto(homeBrowsePath(libraryId));
  }
</script>

<section class="browse-detail item-page">
  <section class="item-hero collection-hero" class:has-artwork={Boolean(heroBackdropUrl)}>
    <div class="detail-art item-poster collection-poster" class:has-image={Boolean(heroBackdropUrl)}>
      {#if kind === 'collection' && collection?.artwork_url}
        <img src={collection.artwork_url} alt={title} />
      {:else}
        <span class="collection-poster-placeholder">
          {title.slice(0, 1).toUpperCase()}
        </span>
      {/if}
    </div>
    <div class="detail-summary item-summary">
      <p class="eyebrow">{eyebrow}</p>
      <h2 class="item-title-fallback">{title}</h2>
      <div class="hero-meta-row">
        <span class="tag">{items.length} title{items.length === 1 ? '' : 's'}</span>
      </div>
      <p class="hero-description">{overview}</p>
      <div class="detail-actions">
        <Button variant="secondary" label="Back" icon="arrow-left" onclick={back} />
      </div>
    </div>
  </section>

  <section class="panel page-panel item-section">
    <div class="section-heading section-heading-actions">
      <h3>Items</h3>
      <span class="muted">{items.length} item{items.length === 1 ? '' : 's'}</span>
    </div>
    {#if catalog.libraryItemsLoading}
      <div class="empty-state tight">Loading library items…</div>
    {:else if items.length === 0}
      <div class="empty-state tight">
        {kind === 'collection'
          ? 'No titles are currently linked to this collection.'
          : kind === 'category'
            ? 'No titles are currently linked to this genre.'
            : 'Playlist creation is planned. Items will appear here when playlists are available.'}
      </div>
    {:else}
      <div class="item-grid hierarchy-item-grid">
        {#each items as item (item.id)}
          <MediaCard {item} />
        {/each}
      </div>
    {/if}
  </section>
</section>

<style>
  /*
   * Component-owned layout glue. The shared .item-hero / .detail-art /
   * .item-summary / .hero-meta-row / .detail-actions / .item-section / .item-grid
   * rules live in app.css (mirrors vanilla style.css). .collection-hero and
   * .collection-poster match vanilla style.css:1541-1552; the placeholder
   * initial is browse-detail-only.
   */
  .browse-detail {
    display: flex;
    flex-direction: column;
    gap: 1.25rem;
    padding-top: 1rem;
    padding-bottom: 1.2rem;
  }

  .collection-hero {
    min-height: min(48vh, 560px);
  }

  .collection-poster {
    position: relative;
  }

  .collection-poster-placeholder {
    font-size: 2.2rem;
    font-weight: 800;
    color: rgba(255, 255, 255, 0.85);
  }
</style>
