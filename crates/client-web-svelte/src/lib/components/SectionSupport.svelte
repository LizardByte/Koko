<script lang="ts">
  // ItemSupport — replaces renderSelectedItemSupportGrid()
  // (../client-web/src/app/itemPersonView.ts:990-1033). Two panels: file &
  // library info, and a metadata panel showing the linked match summary.
  import { formatTimestamp } from '$lib';
  import { libraries } from '$lib/stores';
  import type { MediaItemDetail, ItemMetadataResponse } from '$lib/api';

  type Props = {
    item: MediaItemDetail;
    metadata: ItemMetadataResponse | undefined;
  };
  let { item, metadata }: Props = $props();

  const library = $derived(libraries.byId(item.library_id));
  const primaryMatch = $derived(
    metadata?.matches.find((match) => match.relation_kind === 'primary') ?? metadata?.matches[0],
  );
  // Attribution lives on the provider, not the match — resolve via provider_id.
  const provider = $derived(
    primaryMatch ? metadata?.providers.find((entry) => entry.id === primaryMatch.provider_id) : undefined,
  );
</script>

<section class="item-support-grid">
  <div class="panel page-panel detail-card">
    <div class="section-heading"><h3>File and library</h3></div>
    <div class="item-info-list">
      <div><span class="label">Library</span><span>{library?.name ?? 'Unknown'}</span></div>
      <div><span class="label">Folders</span><span>{library?.paths.length ?? 0}</span></div>
      <div><span class="label">Source</span><span class="mono">{item.relative_path}</span></div>
      <div><span class="label">Updated</span><span>{formatTimestamp(item.modified_at)}</span></div>
    </div>
  </div>

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
</section>

<style>
  /*
   * Component-owned (ItemSupport-only). Values mirror vanilla style.css
   * :1594-1708. .panel / .page-panel / .detail-card / .hero-meta-row / .tag
   * are shared (app.css); .hero-meta-row is used across components but the
   * shared rule lives global, this block adds no override to it.
   */
  .item-support-grid {
    display: grid;
    grid-template-columns: minmax(260px, 360px) minmax(0, 1fr);
    gap: 1rem;
    align-items: start;
  }

  .item-info-list {
    display: grid;
    gap: 0.9rem;
  }

  .item-info-list > div {
    display: flex;
    flex-direction: column;
    gap: 0.2rem;
  }

  .mono {
    font-family: 'Cascadia Mono', 'Fira Code', Consolas, monospace;
    font-size: 0.82rem;
    word-break: break-all;
  }

  .attribution {
    font-size: 0.78rem;
    margin-top: 0.4rem;
  }

  .attribution a {
    color: #9ab1d1;
  }
</style>
