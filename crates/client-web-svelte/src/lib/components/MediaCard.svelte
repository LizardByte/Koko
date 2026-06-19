<script lang="ts">
  // A media card. Replaces the vanilla client's renderItemCard()
  // (../client-web/src/app/homeView.ts) which is re-rendered as a string on
  // every state change. Here it's a declarative component with reactive props
  // and a hover-zoom preview, no domPatcher involvement.
  import Icon from './Icon.svelte';
  import type { MediaItemSummary } from '$lib/api';
  import { formatDuration } from '$lib/format';
  import { getArtworkUrl, isMockApi } from '$lib/api';

  type Props = { item: MediaItemSummary };
  let { item }: Props = $props();

  function iconForItemType(itemType: string): string {
    if (itemType === 'movie' || itemType === 'episode' || itemType === 'video') return 'film';
    if (itemType === 'show' || itemType === 'season') return 'tv';
    if (itemType === 'album' || itemType === 'track' || itemType === 'artist') return 'music';
    return 'layers';
  }

  const artworkTargetId = $derived(item.artwork_item_id ?? item.id);
  const artworkUrl = $derived(getArtworkUrl(artworkTargetId, 'poster', item.artwork_updated_at));
  const progressPct = $derived(
    item.playback_duration_ms && item.playback_position_ms && item.playback_duration_ms > 0
      ? Math.min(100, (item.playback_position_ms / item.playback_duration_ms) * 100)
      : 0,
  );
</script>

<a class="media-card" href="/items/{item.id}" data-item-id={item.id}>
  <div class="poster">
    {#if isMockApi()}
      <div class="poster-placeholder poster-gradient-{item.id % 4}">
        <Icon name={iconForItemType(item.item_type)} size={28} />
      </div>
    {:else}
      <img src={artworkUrl} alt="" loading="lazy" />
    {/if}
    {#if item.playback_position_ms && progressPct > 0 && progressPct < 99}
      <div class="resume-bar"><div class="resume-fill" style="width:{progressPct}%"></div></div>
    {/if}
    <div class="poster-overlay">
      <span class="play-pill"><Icon name="play" size={16} /></span>
    </div>
  </div>
  <div class="meta">
    <div class="title">{item.display_title}</div>
    {#if item.display_subtitle}
      <div class="subtitle muted">{item.display_subtitle}</div>
    {/if}
    {#if item.duration_ms}
      <div class="duration muted">{formatDuration(item.duration_ms)}</div>
    {/if}
  </div>
</a>

<style>
  .media-card {
    display: block;
    text-decoration: none;
    color: inherit;
    width: 150px;
    flex-shrink: 0;
  }
  .poster {
    position: relative;
    aspect-ratio: 2 / 3;
    border-radius: 8px;
    overflow: hidden;
    background: var(--koko-border, #e5e5e5);
    box-shadow: 0 1px 3px rgba(0, 0, 0, 0.15);
    transition: transform 0.15s ease, box-shadow 0.15s ease;
  }
  .media-card:hover .poster {
    transform: translateY(-3px);
    box-shadow: 0 6px 16px rgba(0, 0, 0, 0.3);
  }
  .poster img {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }
  .poster-placeholder {
    width: 100%;
    height: 100%;
    display: flex;
    align-items: center;
    justify-content: center;
    color: rgba(255, 255, 255, 0.85);
  }
  .poster-gradient-0 {
    background: linear-gradient(135deg, #6366f1, #8b5cf6);
  }
  .poster-gradient-1 {
    background: linear-gradient(135deg, #0ea5e9, #06b6d4);
  }
  .poster-gradient-2 {
    background: linear-gradient(135deg, #f97316, #ef4444);
  }
  .poster-gradient-3 {
    background: linear-gradient(135deg, #10b981, #14b8a6);
  }
  .resume-bar {
    position: absolute;
    bottom: 0;
    left: 0;
    right: 0;
    height: 3px;
    background: rgba(0, 0, 0, 0.5);
  }
  .resume-fill {
    height: 100%;
    background: #ef4444;
  }
  .poster-overlay {
    position: absolute;
    inset: 0;
    display: flex;
    align-items: center;
    justify-content: center;
    background: rgba(0, 0, 0, 0.35);
    opacity: 0;
    transition: opacity 0.15s ease;
  }
  .media-card:hover .poster-overlay {
    opacity: 1;
  }
  .play-pill {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 42px;
    height: 42px;
    border-radius: 50%;
    background: rgba(255, 255, 255, 0.9);
    color: #111;
  }
  .meta {
    padding: 0.4rem 0.1rem 0;
  }
  .title {
    font-size: 0.85rem;
    font-weight: 500;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .subtitle {
    font-size: 0.78rem;
    margin-top: 0.1rem;
  }
  .duration {
    font-size: 0.75rem;
    margin-top: 0.1rem;
  }
  .muted {
    color: var(--koko-muted, #777);
  }
</style>
