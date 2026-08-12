<script lang="ts">
  // CardSurface — shared card shell for the rectangular "tile + caption" cards
  // used across the app (MediaCard, PersonCard, MediaExtraCard). Encapsulates
  // the structure + the few CSS declarations that are genuinely identical:
  //   - a transparent <button> root (no bg/shadow/padding; layout-only flex column)
  //   - a rounded, overflow-clipped tile wrapper (the visible surface)
  //
  // Card-specific visuals (aspect ratio, radius, border, gradient, hover
  // behavior, badge overlays) are passed as props/slots so each card keeps its
  // identity. This is the one place to fix card-wide bugs (corner bleed,
  // hover leak) instead of three.
  //
  // Naming taxonomy: unprefixed = leaf/sub-component (see PORTING_GUIDELINES).
  import type { Snippet } from 'svelte';

  type Props = {
    /** Border radius of the TILE (the visible surface), in px. */
    tileRadius: number;
    /** Tile aspect ratio, e.g. '2 / 3' (poster) or '16 / 9' (backdrop). */
    aspectRatio: string;
    /** Show a 1px subtle border on the tile (rgba(255,255,255,0.08)). */
    bordered?: boolean;
    /**
     * Hover behavior:
     *   'tile'  — lift + glow the TILE only (MediaCard blue-shadow lift);
     *             suppresses the global button hover so it doesn't leak.
     *   'none'  — no hover override (inherits global button hover).
     */
    hover?: 'tile' | 'none';
    /** Click handler for the button root. */
    onclick?: () => void;
    /** Extra classes on the root button (e.g. 'episode-card', 'is-missing'). */
    class?: string;
    /** Extra classes on the tile wrapper (e.g. 'media-card-art'). */
    tileClass?: string;
    /** ARIA label for the button root. */
    label?: string;
    /** Slot: the tile's visible content (image, fallback, badges). */
    art?: Snippet;
    /** Slot: the caption block below the tile (title, subtitle, meta). */
    body?: Snippet;
  };

  let {
    tileRadius,
    aspectRatio,
    bordered = false,
    hover = 'none',
    onclick,
    class: klass = '',
    tileClass = '',
    label,
    art,
    body,
  }: Props = $props();
</script>

<button
  type="button"
  class="card-surface {klass}"
  class:hover-tile={hover === 'tile'}
  {onclick}
  aria-label={label}
>
  <span
    class="card-surface-tile {tileClass}"
    class:is-bordered={bordered}
    style="aspect-ratio: {aspectRatio}; border-radius: {tileRadius}px;"
  >
    {@render art?.()}
  </span>
  {#if body}{@render body()}{/if}
</button>

<style>
  /* Transparent root — layout-only flex column. No bg/shadow/padding so the
     tile + caption stack flush, matching vanilla's .media-card/.person-card/
     .media-extra-card roots (style.css :927/:1819/:1745). */
  .card-surface {
    display: flex;
    flex-direction: column;
    align-items: stretch;
    gap: 0.55rem;
    padding: 0;
    background: transparent;
    box-shadow: none;
    text-align: left;
  }

  /* The visible surface. overflow:hidden clips gradient fallbacks + images to
     the rounded corners (a documented Svelte-port delta — vanilla omits this
     and lets mock/oversized art bleed past the radius). */
  .card-surface-tile {
    position: relative;
    display: flex;
    align-items: center;
    justify-content: center;
    overflow: hidden;
    background-size: cover;
    background-position: center;
    background-repeat: no-repeat;
  }
  .card-surface-tile.is-bordered {
    border: 1px solid rgba(255, 255, 255, 0.08);
  }

  /* hover='tile': lift + glow the tile only, suppressing the global button
     hover so the glow doesn't leak past the tile onto the caption (the
     vanilla .media-card leak bug, fixed in one place here). */
  .card-surface.hover-tile:hover:not(:disabled) {
    transform: none;
    box-shadow: none;
    filter: none;
  }
  .card-surface.hover-tile:hover:not(:disabled) .card-surface-tile {
    transform: translateY(-2px);
    box-shadow: 0 16px 32px rgba(93, 123, 255, 0.42);
  }
  .card-surface.hover-tile:focus-visible {
    outline-offset: 4px;
  }
</style>
