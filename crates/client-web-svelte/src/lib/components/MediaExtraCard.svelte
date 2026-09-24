<script lang="ts">
  // MediaExtraCard — a single trailer/theme thumbnail card. Extracted from
  // SectionExtras so the card is independently testable + shares the
  // CardSurface shell with MediaCard/PersonCard.
  // Replaces the inner renderMediaExtraCard() of
  // ../client-web/src/app/itemPersonView.ts:314-359.
  import Icon from './Icon.svelte';
  import CardSurface from './CardSurface.svelte';
  import { resolveApiUrl, type MediaItemExtra } from '$lib/api';

  type Props = { extra: MediaItemExtra; onplay?: (extra: MediaItemExtra) => void };
  let { extra, onplay }: Props = $props();

  const thumbnail = $derived(extra.thumbnail_url ? resolveApiUrl(extra.thumbnail_url) : undefined);
</script>

<CardSurface
  tileRadius={8}
  aspectRatio="16 / 9"
  bordered
  class="media-extra-card"
  label={extra.title ?? extra.extra_type}
  onclick={() => onplay?.(extra)}
>
  {#snippet art()}
    <span
      class="media-extra-thumbnail"
      class:has-image={Boolean(thumbnail)}
      style={thumbnail ? `background-image: url('${thumbnail}');` : ''}
    >
      {#if !thumbnail}
        <span class="media-extra-placeholder-icon">
          <Icon name={extra.extra_type === 'theme_song' ? 'music' : 'play'} size={32} />
        </span>
      {/if}
      <span class="media-extra-play-icon"><Icon name="play" size={16} /></span>
    </span>
  {/snippet}

  {#snippet body()}
    <span class="media-extra-title">{extra.title ?? extra.extra_type}</span>
    <span class="media-extra-meta">
      <span>{extra.extra_type === 'theme_song' ? 'Theme' : 'Trailer'}</span>
      {#if extra.duration_seconds}<span>{Math.floor(extra.duration_seconds / 60)}m</span>{/if}
    </span>
  {/snippet}
</CardSurface>

<style>
  /* Card-specific layout overrides for the shell root. The shell provides the
     transparent button + tile wrapper; here we fix the width + gap to match
     vanilla .media-extra-card (style.css:1745). */
  :global(.media-extra-card.card-surface) {
    width: 244px;
  }

  /* Gradient fallback + placeholder/play icons are card-specific — they live
     here, not on CardSurface. overflow:hidden comes from the shell tile. */
  .media-extra-thumbnail:not(.has-image) {
    background-image: linear-gradient(135deg, rgba(57, 78, 123, 0.88), rgba(12, 18, 32, 0.94));
    width: 100%;
    height: 100%;
    display: grid;
    place-items: center;
    color: #e7f0ff;
  }

  .media-extra-placeholder-icon :global(svg) {
    width: 2rem;
    height: 2rem;
  }

  .media-extra-play-icon {
    position: absolute;
    right: 0.55rem;
    bottom: 0.55rem;
    display: inline-grid;
    place-items: center;
    width: 2rem;
    height: 2rem;
    border-radius: 999px;
    background: rgba(5, 10, 18, 0.72);
    color: #fff;
  }

  .media-extra-title {
    min-height: 2.5rem;
    overflow: hidden;
    display: -webkit-box;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
    font-weight: 700;
    line-height: 1.25;
  }

  .media-extra-meta {
    display: flex;
    justify-content: space-between;
    gap: 0.65rem;
    color: var(--muted);
    font-size: 0.82rem;
  }

  .media-extra-meta span {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
</style>
