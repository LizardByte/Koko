<script lang="ts">
  // Browse-detail route: /items/<kind>/<key> where kind ∈
  // {collections,categories,playlists}. Renders BrowseListing. Port of the
  // vanilla router's browse-detail branch (routes.ts:48-55) — see
  // BROWSE_FILTER_PROPOSAL.md (Option A, no store).
  import { page } from '$app/state';
  import BrowseListing from '$lib/components/BrowseListing.svelte';
  import type { BrowseListingKind } from '$lib/paths';

  const VALID_KINDS: ReadonlySet<string> = new Set(['collections', 'categories', 'playlists']);

  const kindParam = $derived(page.params.kind);
  const key = $derived(page.params.key ?? '');
  const kind = $derived(
    kindParam && VALID_KINDS.has(kindParam)
      ? (kindParam === 'collections'
          ? 'collection'
          : kindParam === 'categories'
            ? 'category'
            : 'playlist') as BrowseListingKind
      : undefined,
  );
</script>

<svelte:head><title>{key ? `${decodeURIComponent(key)} — Koko` : 'Koko'}</title></svelte:head>

{#if kind}
  <BrowseListing {kind} {key} />
{:else}
  <section class="panel page-panel">
    <div class="empty-state">This browse view is not available.</div>
  </section>
{/if}
