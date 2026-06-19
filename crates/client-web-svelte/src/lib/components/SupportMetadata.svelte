<script lang="ts">
  // SupportMetadata — Panel B of SectionSupport: "Link metadata" / "Metadata".
  // Shows the primary provider match summary (tags, overview, attribution) or
  // an empty-state when nothing is linked. Self-contained; derives the primary
  // match + provider internally from the metadata prop.
  // Replaces the right panel of renderSelectedItemSupportGrid()
  // (../client-web/src/app/itemPersonView.ts:990-1033).
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
</script>

<div class="panel page-panel detail-card">
  <div class="section-heading"><h3>{item.item_type === 'movie' || item.item_type === 'show' ? 'Link metadata' : 'Metadata'}</h3></div>
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
