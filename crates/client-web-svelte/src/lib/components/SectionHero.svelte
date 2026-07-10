<script lang="ts">
  // SectionHero — the top summary banner of the item-detail page: poster,
  // title (logo or fallback), tagline, badge row, collapsible overview, action
  // buttons (HeroActions), and technical facts (FactList).
  //
  // "Hero" is an industry-standard web-design term for the large summary
  // banner at the top of a detail/landing page (Netflix, Disney+, Spotify,
  // e-commerce). It's not media-specific jargon.
  //
  // Replaces renderSelectedItemHero() (../client-web/src/app/itemPersonView.ts:
  // 764-909). Actions + facts were split out (mirroring vanilla's separate
  // renderSelectedItemActions / renderSelectedItemFactList functions).
  import CollapsibleText from './CollapsibleText.svelte';
  import Icon from './Icon.svelte';
  import HeroActions from './HeroActions.svelte';
  import FactList from './FactList.svelte';
  import { getArtworkUrl, resolveApiUrl, type MediaItemDetail } from '$lib/api';
  import { playbackProgressPercent } from '$lib/playbackProgress';
  import { formatTimestamp } from '$lib/format';

  type Props = { itemValue: MediaItemDetail };
  let { itemValue }: Props = $props();

  const posterUrl = $derived(
    itemValue.poster_url
      ? getArtworkUrl(itemValue.id, 'poster', itemValue.artwork_updated_at)
      : undefined,
  );
  const logoUrl = $derived(itemValue.logo_url ? resolveApiUrl(itemValue.logo_url) : undefined);
  const isMissing = $derived(Boolean(itemValue.missing_since));
  const genres = $derived(itemValue.genres);

  // Detail-badge state — mirrors playbackDetailBadgeMarkup / missingItemDetail
  // BadgeMarkup (homeView.ts:330-382): watch count + label, progress %, and
  // the missing-since timestamp for the warning tag.
  const watchCount = $derived(itemValue.watch_count ?? 0);
  const watchedLabel = $derived(watchCount === 1 ? 'Watched' : `Watched ${watchCount}x`);
  const watchedTitle = $derived(
    itemValue.last_watched_at
      ? `Last watched ${formatTimestamp(itemValue.last_watched_at)}`
      : watchedLabel,
  );
  // MediaItemDetail allows null on the playback fields where MediaItemSummary
  // uses undefined; coerce to the summary shape playbackProgressPercent wants.
  const summaryForProgress = $derived({
    ...itemValue,
    playback_position_ms: itemValue.playback_position_ms ?? undefined,
    playback_duration_ms: itemValue.playback_duration_ms ?? undefined,
    duration_ms: itemValue.duration_ms ?? undefined,
  });
  const progressPercent = $derived(playbackProgressPercent(summaryForProgress));
  const missingTitle = $derived(`Missing from disk since ${formatTimestamp(itemValue.missing_since ?? undefined)}`);
</script>

<section class="item-hero" class:episode-hero={itemValue.item_type === 'episode'}>
  <div class="detail-art item-poster" class:item-thumbnail={itemValue.item_type === 'episode'} class:has-image={Boolean(posterUrl)}>
    {#if posterUrl}
      <img src={posterUrl} alt="{itemValue.display_title} poster" />
    {:else}
      <span>{itemValue.display_title.slice(0, 1).toUpperCase()}</span>
    {/if}
  </div>
  <div class="detail-summary item-summary">
    {#if logoUrl}
      <img class="item-title-logo" src={logoUrl} alt={itemValue.display_title} />
    {:else}
      <h2 class="item-title-fallback">{itemValue.display_title}</h2>
    {/if}

    {#if itemValue.tagline}
      <p class="hero-tagline">{itemValue.tagline}</p>
    {/if}

    <div class="hero-meta-row">
      {#if isMissing}
        <span class="tag warning status-tag" title={missingTitle} aria-label={missingTitle}>
          <span class="status-icon"><Icon name="triangle-alert" size={15} strokeWidth={2.2} /></span>
          <span>Missing</span>
        </span>
      {/if}
      {#if watchCount > 0}
        <span class="tag success status-tag" title={watchedTitle}>
          <span class="status-icon"><Icon name="circle-check" size={15} strokeWidth={2.2} /></span>
          <span>{watchedLabel}</span>
        </span>
      {/if}
      {#if progressPercent !== undefined}
        <span class="tag status-tag">{progressPercent}% watched</span>
      {/if}
      {#if itemValue.release_year}<span class="tag">{itemValue.release_year}</span>{/if}
      {#if itemValue.content_rating}<span class="tag">{itemValue.content_rating}</span>{/if}
      {#if typeof itemValue.rating === 'number'}<span class="tag">{itemValue.rating.toFixed(1)}</span>{/if}
      {#each genres as genre (genre)}<span class="tag">{genre}</span>{/each}
    </div>

    <CollapsibleText text={itemValue.overview ?? 'No description is stored for this item yet.'} storageKey="item-overview:{itemValue.id}" className="hero-description" />

    <HeroActions {itemValue} />

    <FactList {itemValue} />
  </div>
</section>

<style>
  /*
   * Component-owned (SectionHero-only). Shared .item-hero / .item-poster /
   * .item-summary / .item-title-fallback / .detail-art / .detail-summary /
   * .hero-meta-row / .hero-tagline / .detail-actions live in app.css (used by
   * PersonHero too). Fact-list rules moved to FactList.svelte. Values mirror
   * vanilla style.css:1537-1592.
   */
  .item-hero.episode-hero {
    grid-template-columns: minmax(280px, 360px) minmax(0, 1fr);
  }

  .item-thumbnail {
    width: min(100%, 360px);
    aspect-ratio: 16 / 9;
  }

  .item-title-logo {
    display: block;
    max-width: min(340px, 100%);
    max-height: 120px;
    object-fit: contain;
    object-position: left center;
  }

  @media (max-width: 960px) {
    .item-hero {
      grid-template-columns: minmax(0, 1fr);
    }
    .item-poster {
      width: min(220px, 100%);
    }
  }
</style>
