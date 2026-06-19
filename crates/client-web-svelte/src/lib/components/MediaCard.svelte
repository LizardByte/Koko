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

  // Metadata badges (pending / unmatched). Mirrors metadataBadgeMarkup.
  const isUnmatched = $derived(item.has_metadata === false);
  const isMetadataLoading = $derived(item.metadata_refresh_state === 'pending');
  const hasMetadataBadges = $derived(isUnmatched || isMetadataLoading);
  const hasMultipleMetaBadges = $derived(isUnmatched && isMetadataLoading);
  const hasUnmatchedAndLoading = $derived(hasMultipleMetaBadges);

  // Playback badges: progress donut + watched checkmark.
  const progressPercent = $derived(playbackProgressPercent(item));
  const isWatched = $derived(item.playback_completed === true);
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
        <Icon name={selectedLibraryIcon(library?.kind)} size={16} />
      </span>
      {#if isMissing}
        <span class="media-card-status is-missing" title="Missing from disk" aria-label="Missing">
          <span class="status-icon"><Icon name="triangle-alert" size={14} strokeWidth={2.2} /></span>
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
            {#if isUnmatched}
              <span
                class="media-card-status is-unmatched"
                class:icon-only={!hasMultipleMetaBadges}
                class:has-multiple={hasMultipleMetaBadges}
                title="Metadata is not linked yet"
              >
                <span class="status-warning-icon status-icon"><Icon name="triangle-alert" size={14} strokeWidth={2.2} /></span>
                {#if !hasMultipleMetaBadges}<span>Unmatched</span>{/if}
              </span>
            {/if}
            {#if isMetadataLoading}
              <span
                class="media-card-status is-loading"
                class:icon-only={!isUnmatched}
                class:has-multiple={hasUnmatchedAndLoading}
                title="Refreshing metadata"
                aria-label="Refreshing metadata"
              >
                <span class="loading-spinner is-spinner-visible"></span>
              </span>
            {/if}
          </span>
        {/if}
        {#if hasPlaybackBadges}
          <span class="media-card-playback-badges">
            {#if progressPercent !== undefined}
              <span class="media-card-progress" style="--watch-progress: {progressPercent}%"></span>
            {/if}
            {#if isWatched}
              <span class="media-card-status is-watched icon-only" title="Watched" aria-label="Watched">
                <span class="status-icon"><Icon name="circle-check" size={16} strokeWidth={2.2} /></span>
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
  .media-card {
    display: flex;
    flex-direction: column;
    gap: 0.6rem;
    padding: 0;
    border-radius: 18px;
    background: transparent;
    box-shadow: none;
    text-align: left;
  }

  .media-card.episode-card {
    gap: 0.45rem;
  }

  .media-card-art {
    position: relative;
    aspect-ratio: 2 / 3;
    border-radius: 18px;
    padding: 0.9rem;
    display: block;
    background: linear-gradient(180deg, rgba(93, 123, 255, 0.8), rgba(22, 31, 54, 0.92));
    background-position: center;
    background-size: cover;
    border: 1px solid rgba(255, 255, 255, 0.08);
    overflow: hidden;
  }

  .media-card-art.episode {
    aspect-ratio: 16 / 9;
  }

  .media-card-art.audio {
    background: linear-gradient(180deg, rgba(66, 214, 158, 0.78), rgba(17, 44, 40, 0.94));
  }

  .media-card.is-missing .media-card-art {
    border-color: rgba(255, 191, 84, 0.32);
    box-shadow: inset 0 0 0 1px rgba(255, 191, 84, 0.14);
  }

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

  .media-card-kind-row {
    position: absolute;
    top: 0.85rem;
    right: 0.85rem;
    left: 0.85rem;
    z-index: 2;
    display: flex;
    justify-content: space-between;
    pointer-events: none;
  }

  .media-card-dynamic-badges {
    position: absolute;
    right: 0.85rem;
    bottom: 0.85rem;
    left: 0.85rem;
    z-index: 2;
    display: flex;
    justify-content: space-between;
    pointer-events: none;
  }

  .media-card-state-badges,
  .media-card-playback-badges {
    display: flex;
    align-items: center;
    gap: 0.35rem;
  }

  .media-card-playback-badges {
    margin-left: auto;
    justify-content: flex-end;
  }

  .media-card-kind,
  .media-card-duration {
    padding: 0.35rem 0.55rem;
    border-radius: 999px;
    background: rgba(10, 14, 24, 0.36);
    color: #f4f7fb;
    font-size: 0.76rem;
    white-space: nowrap;
    display: inline-flex;
    align-items: center;
    gap: 0.3rem;
  }

  .media-card-status {
    display: inline-flex;
    align-items: center;
    gap: 0.28rem;
    padding: 0.24rem 0.42rem;
    border-radius: 999px;
    background: rgba(10, 14, 24, 0.52);
    color: #f4f7fb;
    font-size: 0.74rem;
    white-space: nowrap;
  }

  .media-card-status.icon-only {
    padding: 0.24rem;
    min-width: 1.45rem;
    height: 1.45rem;
    justify-content: center;
  }

  .media-card-status.is-unmatched {
    color: #ffe5b5;
  }

  .media-card-status.is-loading {
    color: #dce6ff;
  }

  .media-card-status.is-missing {
    min-width: 1.45rem;
    height: 1.45rem;
    color: #ffd78a;
  }

  .media-card-status.is-watched {
    background: rgba(10, 14, 24, 0.58);
    color: #8bf3ca;
  }

  .status-warning-icon {
    color: #ffe5b5;
  }

  .media-card-progress {
    --watch-progress: 0%;
    position: relative;
    display: inline-grid;
    place-items: center;
    width: 1.96rem;
    min-width: 1.96rem;
    height: 1.96rem;
    border-radius: 999px;
    background: conic-gradient(#8bf3ca var(--watch-progress), rgba(255, 255, 255, 0.18) 0);
    color: #f4f7fb;
    font-size: 0.62rem;
    font-weight: 800;
  }

  .media-card-progress::before {
    content: '';
    position: absolute;
    inset: 0.25rem;
    border-radius: inherit;
    background: rgba(10, 14, 24, 0.82);
  }

  .media-card-title {
    font-weight: 700;
    color: #f4f7fb;
  }

  .media-card-subtitle {
    font-size: 0.82rem;
    color: #d8e5ff;
  }
</style>
