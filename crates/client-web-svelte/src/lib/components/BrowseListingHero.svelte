<script lang="ts">
  // BrowseListingHero — the top banner of a browse-listing page (collection,
  // category, or playlist): poster/initial, eyebrow, title, item-count tag,
  // overview, and a Back button. Purely presentational — data is resolved by
  // the parent BrowseListing and passed as props.
  // Replaces the hero section of renderCollectionDetailPage /
  // renderCategoryDetailPage / renderPlaylistDetailPage
  // (../client-web/src/app/homeView.ts:153-278).
  import Button from './Button.svelte';
  import type { Snippet } from 'svelte';

  type Props = {
    eyebrow: string;
    title: string;
    itemCount: number;
    overview: string;
    posterUrl?: string;
    onBack: () => void;
    /** Optional custom poster snippet (e.g. collection artwork <img>). */
    poster?: Snippet;
  };
  let { eyebrow, title, itemCount, overview, posterUrl, onBack, poster }: Props = $props();
</script>

<section class="item-hero collection-hero" class:has-artwork={Boolean(posterUrl)}>
  <div class="detail-art item-poster collection-poster" class:has-image={Boolean(posterUrl)}>
    {#if poster}
      {@render poster()}
    {:else if posterUrl}
      <img src={posterUrl} alt={title} />
    {:else}
      <span class="collection-poster-placeholder">
        {title.slice(0, 1).toUpperCase()}
      </span>
    {/if}
  </div>
  <div class="detail-summary item-summary">
    <p class="eyebrow">{eyebrow}</p>
    <h2 class="item-title-fallback">{title}</h2>
    <div class="hero-meta-row">
      <span class="tag">{itemCount} title{itemCount === 1 ? '' : 's'}</span>
    </div>
    <p class="hero-description">{overview}</p>
    <div class="detail-actions">
      <Button variant="secondary" label="Back" icon="arrow-left" onclick={onBack} />
    </div>
  </div>
</section>

<style>
  .collection-hero {
    min-height: min(48vh, 560px);
  }

  .collection-poster {
    position: relative;
  }

  .collection-poster-placeholder {
    font-size: 2.2rem;
    font-weight: 800;
    color: rgba(255, 255, 255, 0.85);
  }
</style>
