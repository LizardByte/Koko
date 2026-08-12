<script lang="ts">
  // Browse-detail route: /items/<kind>/<key> where kind ∈
  // {collections,categories,playlists}. Renders BrowseListing. Port of the
  // vanilla router's browse-detail branch (routes.ts:48-55) — see
  // BROWSE_FILTER_PROPOSAL.md (Option A, no store).
  import { page } from '$app/state';
  import BrowseListing from '$lib/components/BrowseListing.svelte';
  import { browseKindFromSegment } from '$lib/paths';

  const kindParam = $derived(page.params.kind);
  const key = $derived(page.params.key ?? '');
  const kind = $derived(browseKindFromSegment(kindParam));
</script>

<svelte:head><title>{key ? `${decodeURIComponent(key)} — Koko` : 'Koko'}</title></svelte:head>

{#if kind}
  <BrowseListing {kind} {key} />
{:else}
  <section class="panel page-panel">
    <div class="empty-state">This browse view is not available.</div>
  </section>
{/if}
