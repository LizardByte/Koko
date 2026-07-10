<script lang="ts">
  // Library-scoped browse-detail route:
  // /libraries/<id>/items/<kind>/<key>. Same as /items/<kind>/<key> but
  // scopes the BrowseListing data to a library (passes libraryId). Port of
  // vanilla routes.ts:38-46 (libraryBrowseMatch).
  import { page } from '$app/state';
  import BrowseListing from '$lib/components/BrowseListing.svelte';
  import { browseKindFromSegment } from '$lib/paths';

  const kindParam = $derived(page.params.kind);
  const key = $derived(page.params.key ?? '');
  const libraryId = $derived(Number(page.params.id));
  const kind = $derived(browseKindFromSegment(kindParam));
</script>

<svelte:head><title>{key ? `${decodeURIComponent(key)} — Koko` : 'Koko'}</title></svelte:head>

{#if kind && Number.isFinite(libraryId)}
  <BrowseListing {kind} {key} {libraryId} />
{:else}
  <section class="panel page-panel">
    <div class="empty-state">This browse view is not available.</div>
  </section>
{/if}
