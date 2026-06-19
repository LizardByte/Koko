<script lang="ts">
  // Library-scoped browse-detail route:
  // /libraries/<id>/items/<kind>/<key>. Same as /items/<kind>/<key> but
  // scopes the BrowseDetail data to a library (passes libraryId). Port of
  // vanilla routes.ts:38-46 (libraryBrowseMatch).
  import { page } from '$app/state';
  import BrowseDetail from '$lib/components/BrowseDetail.svelte';
  import type { BrowseDetailKind } from '$lib/paths';

  const VALID_KINDS: ReadonlySet<string> = new Set(['collections', 'categories', 'playlists']);

  const kindParam = $derived(page.params.kind);
  const key = $derived(page.params.key ?? '');
  const libraryId = $derived(Number(page.params.id));
  const kind = $derived(
    kindParam && VALID_KINDS.has(kindParam)
      ? (kindParam === 'collections'
          ? 'collection'
          : kindParam === 'categories'
            ? 'category'
            : 'playlist') as BrowseDetailKind
      : undefined,
  );
</script>

<svelte:head><title>{key ? `${decodeURIComponent(key)} — Koko` : 'Koko'}</title></svelte:head>

{#if kind && Number.isFinite(libraryId)}
  <BrowseDetail {kind} {key} {libraryId} />
{:else}
  <section class="panel page-panel">
    <div class="empty-state">This browse view is not available.</div>
  </section>
{/if}
