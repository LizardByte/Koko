<script lang="ts">
  // HomeFeature — replaces renderHomeFeature() (../client-web/src/app/
  // homeView.ts:470-519). Two variants: collection hero and item hero. Both
  // use the --home-feature-image technique: a ::before pseudo-element holds
  // the backdrop with a right-side mask (only the left ~18% fades), and a
  // ::after pseudo-element adds a left-side dark scrim. isolation:isolate
  // keeps the layers from bleeding through. The CSS lives here (scoped) so it
  // travels with the component.
  import Button from './Button.svelte';
  import { goto } from '$app/navigation';
  import {
    getArtworkUrl,
    type MediaCollectionSummary,
    type MediaItemSummary,
  } from '$lib/api';
  import { libraries } from '$lib/stores';
  import { formatChildCount, humanizeItemType } from '$lib/ui';
  import { resolveApiUrl } from '$lib/api';

  type Props = {
    collection?: MediaCollectionSummary;
    item?: MediaItemSummary;
  };
  let { collection, item }: Props = $props();

  // Backdrop URL: collection uses artwork/backdrop via resolveApiUrl; item uses
  // getArtworkUrl(backdrop) when item.backdrop_url is set (pageBackdropUrlForItem
  // semantics). In mock mode these are placeholders, so has-artwork stays off
  // and the hero renders a solid base — matching the vanilla client's
  // broken-image behavior.
  const backdropUrl = $derived(
    collection
      ? collection.backdrop_url
        ? resolveApiUrl(collection.backdrop_url)
        : collection.artwork_url
          ? resolveApiUrl(collection.artwork_url)
          : undefined
      : item?.backdrop_url
        ? getArtworkUrl(item.id, 'backdrop', item.artwork_updated_at)
        : undefined,
  );
  const logoUrl = $derived(item?.logo_url ? getArtworkUrl(item.id, 'logo', item.artwork_updated_at) : undefined);
  const libraryName = $derived(item ? libraries.byId(item.library_id)?.name ?? 'your library' : '');
</script>

{#if collection}
  <section
    class="home-feature"
    class:has-artwork={Boolean(backdropUrl)}
    style={backdropUrl ? `--home-feature-image: url('${backdropUrl}');` : ''}
  >
    <div class="home-feature-copy">
      <p class="eyebrow">Collection</p>
      <h2>{collection.name}</h2>
      <p>{collection.overview ?? `${collection.item_count} title${collection.item_count === 1 ? '' : 's'} in this collection.`}</p>
      <div class="hero-meta-row">
        <span class="tag">{collection.item_count} title{collection.item_count === 1 ? '' : 's'}</span>
      </div>
    </div>
    <Button variant="secondary" icon="arrow-right" class="home-feature-action" label="Open" onclick={() => goto(`/collections/${collection.id}`)} />
  </section>
{:else if item}
  <section
    class="home-feature"
    class:has-artwork={Boolean(backdropUrl)}
    style={backdropUrl ? `--home-feature-image: url('${backdropUrl}');` : ''}
  >
    <div class="home-feature-copy">
      {#if logoUrl}
        <img class="home-feature-logo" src={logoUrl} alt={item.display_title} />
      {:else}
        <h2>{item.display_title}</h2>
      {/if}
      <p>{item.overview ?? `${humanizeItemType(item.item_type)} from ${libraryName}.`}</p>
      <div class="hero-meta-row">
        {#each item.genres.slice(0, 3) as genre (genre)}
          <span class="tag">{genre}</span>
        {/each}
        <span class="tag">{formatChildCount(item)}</span>
      </div>
    </div>
    <Button variant="secondary" icon="arrow-right" class="home-feature-action" label="Open" onclick={() => goto(`/items/${item.id}`)} />
  </section>
{/if}

<style>
  /*
   * Component-owned (HomeFeature-only). Values mirror vanilla style.css
   * :1143-1255. The .home-page-backdrop overrides are :global because that
   * class is applied by the page layout (an ancestor), not this component.
   */
  .home-feature {
    position: sticky;
    top: 3.75rem;
    z-index: 7;
    isolation: isolate;
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto;
    align-items: end;
    gap: 1rem;
    height: 350px;
    min-height: 350px;
    max-height: 350px;
    padding: 1.4rem;
    border-radius: 0;
    overflow: hidden;
    border-bottom: 1px solid rgba(255, 255, 255, 0.2);
    background: #0d1221;
  }

  .home-feature.has-artwork {
    background: #0a0f1c;
  }

  .home-feature.has-artwork::before {
    content: '';
    position: absolute;
    inset: 0 0 0 auto;
    z-index: 0;
    width: min(72%, 1040px);
    background-image: var(--home-feature-image);
    background-position: top right;
    background-repeat: no-repeat;
    background-size: cover;
    pointer-events: none;
    mask-image: linear-gradient(to right, transparent 0%, black 18%);
    -webkit-mask-image: linear-gradient(to right, transparent 0%, black 18%);
  }

  .home-feature::after {
    content: '';
    position: absolute;
    inset: 0;
    z-index: 1;
    background: linear-gradient(90deg, rgba(8, 12, 20, 0.95) 0%, rgba(8, 12, 20, 0.72) 28%, rgba(8, 12, 20, 0.18) 52%, transparent 68%);
    pointer-events: none;
  }

  /* Ancestor-class overrides — layout owns .home-page-backdrop, not us. */
  :global(.home-page-backdrop) .home-feature {
    background: #0a0e1c;
  }

  :global(.home-page-backdrop) .home-feature.has-artwork {
    background: #090d1a;
  }

  :global(.home-page-backdrop) .home-feature.has-artwork::before {
    opacity: 0.62;
  }

  :global(.home-page-backdrop) .home-feature::after {
    background: linear-gradient(90deg, rgba(8, 12, 20, 0.9) 0%, rgba(8, 12, 20, 0.64) 28%, rgba(8, 12, 20, 0.12) 52%, transparent 68%);
  }

  .home-feature-copy {
    position: relative;
    z-index: 2;
    display: flex;
    flex-direction: column;
    gap: 0.75rem;
    max-width: 760px;
    min-width: 0;
  }

  .home-feature-copy h2 {
    margin: 0;
    font-size: 2.35rem;
    line-height: 1;
  }

  .home-feature-copy p {
    max-width: 68ch;
    margin: 0;
    color: #d7e4ff;
    display: -webkit-box;
    overflow: hidden;
    -webkit-line-clamp: 3;
    line-clamp: 3;
    -webkit-box-orient: vertical;
  }

  .home-feature-logo {
    width: min(420px, 70%);
    max-height: 135px;
    object-fit: contain;
    object-position: left center;
  }

  /*
   * .home-feature-action is applied to the Open <Button> (rendered by the
   * Button child component), so the selector must reach across the component
   * boundary. .home-feature-actions (a multi-button wrapper) isn't rendered by
   * this port — the hero shows a single Open button — so it's omitted.
   */
  :global(.home-feature-action) {
    position: relative;
    z-index: 2;
    align-self: end;
  }

  @media (max-width: 960px) {
    .home-feature {
      top: 5.6rem;
      height: 230px;
      min-height: 230px;
      max-height: 230px;
    }
  }
</style>
