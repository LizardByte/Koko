<script lang="ts">
  // SupportMetadata — Panel B of SectionSupport: "Link metadata" / "Metadata".
  // Shows the primary provider match summary (refresh-state tags, overview,
  // attribution, last-refreshed timestamp, refresh error) or an empty-state
  // when nothing is linked. For movies/shows, also renders a "Force refresh"
  // button + the interactive MetadataSearchPanel. Seasons/episodes show the
  // inherited-metadata empty-state.
  // Replaces renderLinkedMetadataSummary (itemPersonView.ts:162-223) +
  // renderSelectedItemSupportGrid (itemPersonView.ts:990-1033).
  import Button from './Button.svelte';
  import MetadataSearchPanel from './MetadataSearchPanel.svelte';
  import { item as itemStore, ui, activities } from '$lib/stores';
  import { canManuallyLinkMetadata } from '$lib/selectors';
  import { formatTimestamp } from '$lib/format';
  import type { MediaItemDetail, ItemMetadataResponse } from '$lib/api';

  type Props = { item: MediaItemDetail; metadata: ItemMetadataResponse | undefined };
  let { item, metadata }: Props = $props();

  const primaryMatch = $derived(
    metadata?.matches.find((match) => match.relation_kind === 'primary') ?? metadata?.matches[0],
  );
  const provider = $derived(
    primaryMatch ? metadata?.providers.find((entry) => entry.id === primaryMatch.provider_id) : undefined,
  );
  const supportsManualLinking = $derived(canManuallyLinkMetadata(item));

  // Refresh-state derivation (vanilla itemPersonView.ts:171-184 + activities.ts:33-39).
  // 'pending' = metadata_refresh_state pending + an active activity for this item.
  // 'stalled' = pending but no active worker.
  const hasActiveRefresh = $derived(
    item.metadata_refresh_state === 'pending' &&
    (activities.systemActivities?.activities ?? []).some(
      (a) => a.category === 'metadata_refresh' && a.item_ids.includes(item.id) && a.state !== 'completed' && a.state !== 'failed',
    ),
  );
  const isRefreshing = $derived(primaryMatch?.refresh_state === 'pending' || hasActiveRefresh);
  const refreshStateLabel = $derived.by(() => {
    if (!primaryMatch) return '';
    if (isRefreshing) return 'Refreshing';
    if (primaryMatch.refresh_state === 'pending' || item.metadata_refresh_state === 'pending') return 'Pending without worker';
    if (primaryMatch.refresh_state === 'error') return 'Refresh failed';
    if (primaryMatch.refresh_state === 'fresh') return 'Up to date';
    return '';
  });
  const refreshStateClass = $derived.by(() => {
    if (!refreshStateLabel) return '';
    if (refreshStateLabel === 'Refresh failed') return 'danger-tag';
    if (refreshStateLabel === 'Refreshing' || refreshStateLabel === 'Pending without worker') return 'warning';
    if (refreshStateLabel === 'Up to date') return 'success';
    return '';
  });

  async function forceRefresh() {
    try {
      await itemStore.refreshMetadata(item.id);
      ui.clearError();
    } catch (err) {
      ui.setError(err instanceof Error ? err.message : 'Failed to refresh media metadata.');
    }
  }
</script>

<div class="panel page-panel detail-card">
  <div class="section-heading section-heading-actions">
    <div><h3>{supportsManualLinking ? 'Link metadata' : 'Metadata'}</h3></div>
    {#if supportsManualLinking}
      <Button
        variant="secondary"
        label={isRefreshing ? 'Refreshing metadata' : 'Force refresh metadata'}
        icon="refresh-cw"
        busy={isRefreshing}
        onclick={forceRefresh}
      />
    {/if}
  </div>
  {#if primaryMatch}
    <div class="linked-metadata-summary">
      <div class="hero-meta-row">
        <span class="tag">{primaryMatch.provider_id}</span>
        {#if primaryMatch.release_year}<span class="tag">{primaryMatch.release_year}</span>{/if}
        {#if refreshStateLabel}<span class="tag {refreshStateClass}">{refreshStateLabel}</span>{/if}
      </div>
      {#if primaryMatch.overview}<p class="muted">{primaryMatch.overview}</p>{/if}
      {#if primaryMatch.refresh_error}<p class="metadata-refresh-error">{primaryMatch.refresh_error}</p>{/if}
      <p class="muted attribution">
        {#if provider?.attribution_url}<a href={provider.attribution_url} target="_blank" rel="noreferrer">{provider.attribution_text ?? provider.attribution_url}</a>{:else if provider?.attribution_text}{provider.attribution_text}{/if}
      </p>
      {#if primaryMatch.last_refreshed_at ?? primaryMatch.updated_at}
        <p class="muted">Last refreshed {formatTimestamp(primaryMatch.last_refreshed_at ?? primaryMatch.updated_at)}</p>
      {/if}
    </div>
  {:else}
    <div class="empty-state tight">No metadata is linked to this item yet.</div>
  {/if}

  {#if supportsManualLinking}
    <MetadataSearchPanel itemValue={item} {metadata} />
  {:else}
    <div class="empty-state tight">Season and episode metadata is inherited and refreshed automatically from the linked show.</div>
  {/if}
</div>

<style>
  .attribution {
    font-size: 0.78rem;
    margin-top: 0.4rem;
  }

  .attribution a {
    color: #9ab1d1;
  }

  .metadata-refresh-error {
    margin-top: 0.3rem;
    font-size: 0.82rem;
    color: #ffb9b9;
  }
</style>
