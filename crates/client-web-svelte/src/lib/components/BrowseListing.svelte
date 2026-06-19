<script lang="ts">
  // BrowseListing — a routed page-level item list for a browse target
  // (collection, category/genre, or playlist). Resolves data from the catalog
  // store + route params, then renders BrowseListingHero + BrowseListingGrid.
  //
  // Replaces renderBrowseDetailPage() + the collection/category/playlist
  // sub-renderers (../client-web/src/app/homeView.ts:112-295). The three kinds
  // share the same structure (hero + grid); only copy + data source differ, so
  // we keep a single dispatcher rather than three kind-components. No
  // browseFilter state (vanilla's is dead code; see BROWSE_FILTER_PROPOSAL.md).
  import BrowseListingHero from './BrowseListingHero.svelte';
  import BrowseListingGrid from './BrowseListingGrid.svelte';
  import { goto } from '$app/navigation';
  import { catalog } from '$lib/stores';
  import {
    categorySummaries,
    itemsForCollection,
    pageBackdropUrlForCollection,
  } from '$lib/selectors';
  import { homeBrowsePath, type BrowseListingKind } from '$lib/paths';

  type Props = {
    kind: BrowseListingKind;
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
  const collectionArtworkUrl = $derived(
    kind === 'collection' ? collection?.artwork_url : undefined,
  );

  // Back target — the home browse path for the active library (or all).
  function back() {
    goto(homeBrowsePath(libraryId));
  }
</script>

<section class="browse-detail item-page">
  <BrowseListingHero
    {eyebrow}
    {title}
    itemCount={items.length}
    {overview}
    posterUrl={collectionArtworkUrl ?? heroBackdropUrl}
    onBack={back}
  />

  <BrowseListingGrid {items} loading={catalog.libraryItemsLoading} {kind} />
</section>

<style>
  /* Layout glue only. Hero + grid rules moved to their components. The shared
     .item-page / .panel / .item-section / .item-grid rules live in app.css. */
  .browse-detail {
    display: flex;
    flex-direction: column;
    gap: 1.25rem;
    padding-top: 1rem;
    padding-bottom: 1.2rem;
  }
</style>
