<script lang="ts">
  // MediaCard — replaces renderItemCard() (../client-web/src/app/homeView.ts:
  // 432-468) and its badge helpers. Reproduces every variant: episode 16:9
  // layout, missing-item amber border, watched checkmark, progress donut,
  // metadata pending/unmatched badges, artwork resolution via
  // artwork_item_id ?? id, and the secondary meta line (library · type).
  import Icon from './Icon.svelte';
  import { goto } from '$app/navigation';
  import { getArtworkUrl, isMockApi, type MediaItemSummary } from '$lib/api';
  import { libraries, catalog } from '$lib/stores';
  import {
    formatChildCount,
    humanizeItemType,
    itemCardSubtitle,
    selectedLibraryIcon,
  } from '$lib/ui';
  import { playbackProgressPercent } from '$lib/playbackProgress';
  import { formatTimestamp } from '$lib/format';
  import { page } from '$app/state';

  type Props = { item: MediaItemSummary };
  let { item }: Props = $props();

  const library = $derived(libraries.byId(item.library_id));
  const artworkItemId = $derived(item.artwork_item_id ?? item.id);
  const artworkUrl = $derived(getArtworkUrl(artworkItemId, 'poster', item.artwork_updated_at));
  const hasAlternateArtwork = $derived(
    typeof item.artwork_item_id === 'number' && item.artwork_item_id !== item.id,
  );
  const useEpisodeLayout = $derived(item.item_type === 'episode' && !hasAlternateArtwork);
  const artworkTypeClass = $derived(useEpisodeLayout ? item.item_type : 'poster-art');
  const subtitle = $derived(itemCardSubtitle(item));

  // Secondary meta line: suppressed when viewing a season's episodes on the
  // item page; else on a specific-library home it's just the type, otherwise
  // "<library> · <type>".
  const isSeasonEpisodeCard = $derived(
    page.url.pathname.startsWith('/items/') && item.item_type === 'episode',
  );
  const onLibraryHome = $derived(
    page.url.pathname === '/' && catalog.activeLibraryId !== undefined,
  );
  const secondaryMeta = $derived(
    isSeasonEpisodeCard
      ? undefined
      : onLibraryHome
        ? humanizeItemType(item.item_type)
        : `${library?.name ?? 'Library'} · ${humanizeItemType(item.item_type)}`,
  );

  // Metric: missing badge if missing_since, else child-count/duration.
  const isMissing = $derived(Boolean(item.missing_since));
  const missingTitle = $derived(
    item.missing_since ? `Missing from disk since ${formatTimestamp(item.missing_since)}` : 'Missing from disk',
  );

  // Metadata badges (pending / unmatched). Mirrors metadataBadgeMarkup
  // (homeView.ts:297-314): a single status span whose classes depend on state.
  // Vanilla uses the falsy `!item.has_metadata` (so undefined → unmatched).
  // We use the stricter `!== true` — behaviorally identical for the valid
  // domain (boolean | undefined) but won't misread a hypothetical 0/''/null
  // as matched. has_metadata?: boolean is optional, so absent means the item
  // hasn't been linked yet.
  const isUnmatched = $derived(item.has_metadata !== true);
  const isMetadataLoading = $derived(item.metadata_refresh_state === 'pending');
  const hasMetadataBadges = $derived(isUnmatched || isMetadataLoading);
  const hasMultipleMetaBadges = $derived(isUnmatched && isMetadataLoading);
  // Title/aria + class string mirror vanilla's conditional string build.
  const metadataStatusLabel = $derived(
    isMetadataLoading
      ? isUnmatched
        ? 'Matching metadata'
        : 'Refreshing metadata'
      : 'Metadata is not linked yet',
  );
  const metadataStatusClass = $derived(
    [
      'media-card-status',
      isUnmatched ? 'is-unmatched' : '',
      isMetadataLoading ? 'is-loading' : '',
      hasMultipleMetaBadges ? 'has-multiple' : 'icon-only',
    ]
      .filter(Boolean)
      .join(' '),
  );

  // Playback badges: progress donut + watched checkmark. Mirrors
  // playbackStatusBadgeMarkup (homeView.ts:344-365) — the card watched badge
  // keys off watch_count > 0 (NOT playback_completed), and the title carries
  // the count ("Watched" / "Watched N times").
  const progressPercent = $derived(playbackProgressPercent(item));
  const watchCount = $derived(item.watch_count ?? 0);
  const isWatched = $derived(watchCount > 0);
  const watchedLabel = $derived(watchCount === 1 ? 'Watched' : `Watched ${watchCount} times`);
  const hasPlaybackBadges = $derived(progressPercent !== undefined || isWatched);

  // In mock mode, artwork URLs are placeholders; render a gradient fallback
  // so cards aren't broken images.
  const mock = $derived(isMockApi());

  function open() {
    goto(`/items/${item.id}`);
  }
</script>

<button
  type="button"
  class="media-card"
  class:episode-card={useEpisodeLayout}
  class:is-missing={isMissing}
  onclick={open}
>
  <span
    class="media-card-art {item.media_kind} {artworkTypeClass}"
    style={mock ? '' : `background-image: url('${artworkUrl}');`}
  >
    {#if mock}
      <span class="media-card-art-fallback fallback-{item.id % 5} {artworkTypeClass}">
        <Icon name={selectedLibraryIcon(library?.kind)} size={28} />
      </span>
    {/if}
    <span class="media-card-kind-row">
      <span class="media-card-kind">
        <span class="card-icon"><Icon name={selectedLibraryIcon(library?.kind)} /></span>
      </span>
      {#if isMissing}
        <span class="media-card-status is-missing" title={missingTitle} aria-label={missingTitle}>
          <span class="status-icon"><Icon name="triangle-alert" /></span>
          <span>Missing</span>
        </span>
      {:else}
        <span class="media-card-duration">{formatChildCount(item)}</span>
      {/if}
    </span>
    {#if hasMetadataBadges || hasPlaybackBadges}
      <span class="media-card-dynamic-badges">
        {#if hasMetadataBadges}
          <span class="media-card-state-badges">
            <span class={metadataStatusClass} title={metadataStatusLabel} aria-label={metadataStatusLabel}>
              {#if isUnmatched}
                <span class="status-warning-icon"><span class="status-icon"><Icon name="triangle-alert" /></span></span>
              {/if}
              {#if isMetadataLoading}
                <!-- Vanilla adds is-spinner-visible via a global IntersectionObserver
                     (spinners.ts); the Svelte port doesn't replicate that observer yet,
                     so we add the class directly so the spinner animates. -->
                <span class="loading-spinner is-spinner-visible"></span>
              {/if}
            </span>
          </span>
        {/if}
        {#if hasPlaybackBadges}
          <span class="media-card-playback-badges">
            {#if progressPercent !== undefined}
              <span class="media-card-progress" style="--watch-progress: {progressPercent}%"></span>
            {/if}
            {#if isWatched}
              <span class="media-card-status is-watched icon-only" title={watchedLabel} aria-label={watchedLabel}>
                <span class="status-icon"><Icon name="circle-check" /></span>
              </span>
            {/if}
          </span>
        {/if}
      </span>
    {/if}
  </span>
  <span class="media-card-title">{item.display_title}</span>
  {#if subtitle}<span class="media-card-subtitle">{subtitle}</span>{/if}
  {#if secondaryMeta}<span class="media-card-meta">{secondaryMeta}</span>{/if}
</button>

<style>
  /*
   * Only the mock-data artwork fallbacks live here — they have no vanilla
   * counterpart (vanilla lets mock image URLs break; we render a colored
   * gradient tile keyed by item id instead). Every other rule the card uses
   * (.media-card, .media-card-art, .media-card-kind-row, .media-card-status *,
   * .media-card-progress, .status-icon, .card-icon, etc.) lives in app.css,
   * mirroring vanilla style.css:927-1141 — see PORTING_GUIDELINES.md.
   *
   * Clipping: the parent .media-card-art has overflow:hidden (a documented
   * Svelte-port delta — vanilla omits it) so this absolute fallback can't
   * bleed past the rounded corners.
   */
  .media-card-art-fallback {
    position: absolute;
    inset: 0;
    display: grid;
    place-items: center;
    color: rgba(255, 255, 255, 0.85);
  }

  .media-card-art-fallback.poster-art {
    aspect-ratio: auto;
  }

  .fallback-0 {
    background: linear-gradient(135deg, #6366f1, #8b5cf6);
  }
  .fallback-1 {
    background: linear-gradient(135deg, #0ea5e9, #06b6d4);
  }
  .fallback-2 {
    background: linear-gradient(135deg, #f97316, #ef4444);
  }
  .fallback-3 {
    background: linear-gradient(135deg, #10b981, #14b8a6);
  }
  .fallback-4 {
    background: linear-gradient(135deg, #ec4899, #8b5cf6);
  }
</style>
