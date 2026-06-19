<script lang="ts">
  // SupportMetadata — Panel B of SectionSupport: "Link metadata" / "Metadata".
  // Shows the primary provider match summary (tags, overview, attribution) or
  // an empty-state when nothing is linked. For movies/shows, also renders a
  // "Force refresh" button + the interactive MetadataSearchPanel (search +
  // manual link). Seasons/episodes show the inherited-metadata empty-state.
  // Replaces the right panel of renderSelectedItemSupportGrid()
  // (../client-web/src/app/itemPersonView.ts:990-1033).
  import Button from './Button.svelte';
  import MetadataSearchPanel from './MetadataSearchPanel.svelte';
  import { item as itemStore, ui } from '$lib/stores';
  import { canManuallyLinkMetadata } from '$lib/selectors';
  import type { MediaItemDetail, ItemMetadataResponse } from '$lib/api';

  type Props = { item: MediaItemDetail; metadata: ItemMetadataResponse | undefined };
  let { item, metadata }: Props = $props();

  const primaryMatch = $derived(
    metadata?.matches.find((match) => match.relation_kind === 'primary') ?? metadata?.matches[0],
  );
  // Attribution lives on the provider, not the match — resolve via provider_id.
  const provider = $derived(
    primaryMatch ? metadata?.providers.find((entry) => entry.id === primaryMatch.provider_id) : undefined,
  );
  const supportsManualLinking = $derived(canManuallyLinkMetadata(item));
  const isRefreshing = $derived(primaryMatch?.refresh_state === 'pending');

  async function forceRefresh() {
    try {
      await itemStore.refreshMetadata(item.id);
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
        {#if primaryMatch.match_state}<span class="tag">{primaryMatch.match_state}</span>{/if}
      </div>
      {#if primaryMatch.overview}<p class="muted">{primaryMatch.overview}</p>{/if}
      {#if provider?.attribution_text || provider?.attribution_url}
        <p class="muted attribution">
          {#if provider?.attribution_url}<a href={provider.attribution_url} target="_blank" rel="noreferrer">{provider.attribution_text ?? provider.attribution_url}</a>{:else}{provider?.attribution_text}{/if}
        </p>
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
</style>
