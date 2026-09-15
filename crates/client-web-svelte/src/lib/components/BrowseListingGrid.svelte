<script lang="ts">
  // BrowseListingGrid — the items panel of a browse-listing page: heading,
  // loading state, empty state (per kind), or the MediaCard grid. Purely
  // presentational — items + loading flag are passed as props.
  // Replaces the items section of renderCollectionDetailPage /
  // renderCategoryDetailPage / renderPlaylistDetailPage
  // (../client-web/src/app/homeView.ts:153-278).
  import MediaCard from './MediaCard.svelte';
  import type { MediaItemSummary } from '$lib/api';
  import type { BrowseListingKind } from '$lib/paths';

  type Props = {
    items: MediaItemSummary[];
    loading: boolean;
    kind: BrowseListingKind;
  };
  let { items, loading, kind }: Props = $props();

  const EMPTY_MESSAGES: Record<BrowseListingKind, string> = {
    collection: 'No titles are currently linked to this collection.',
    category: 'No titles are currently linked to this genre.',
    playlist: 'Playlist creation is planned. Items will appear here when playlists are available.',
  };
  const emptyMessage = $derived(EMPTY_MESSAGES[kind]);
</script>

<section class="panel page-panel item-section">
  <div class="section-heading section-heading-actions">
    <h3>Items</h3>
    <span class="muted">{items.length} item{items.length === 1 ? '' : 's'}</span>
  </div>
  {#if loading}
    <div class="empty-state tight">Loading library items…</div>
  {:else if items.length === 0}
    <div class="empty-state tight">{emptyMessage}</div>
  {:else}
    <div class="item-grid hierarchy-item-grid">
      {#each items as item (item.id)}
        <MediaCard {item} />
      {/each}
    </div>
  {/if}
</section>
