<script lang="ts">
  // Shelf — replaces renderShelfStack()/renderShelf() + the scroll/lazy logic
  // in eventBindings.ts (updateShelfScrollControls / appendLazyShelfItems).
  // Native CSS grid auto-flow column (no scroll-snap — vanilla doesn't use it),
  // scroll buttons that hide when not scrollable, and lazy card expansion that
  // renders HOME_SHELF_CHUNK_SIZE cards initially and appends more as the user
  // scrolls near the end.
  import MediaCard from './MediaCard.svelte';
  import Icon from './Icon.svelte';
  import { HOME_SHELF_CHUNK_SIZE } from '$lib';
  import type { MediaItemSummary } from '$lib/api';

  type Props = {
    title: string;
    items: MediaItemSummary[];
    id?: string;
    rowCountId?: string;
  };
  let { title, items, id, rowCountId }: Props = $props();

  let scroller: HTMLDivElement | undefined = $state();
  let canScrollLeft = $state(false);
  let canScrollRight = $state(false);
  // Number of cards currently rendered — starts at the chunk size and grows as
  // the user scrolls. Matches data-lazy-rendered-count in the vanilla client.
  let renderedCount = $state(HOME_SHELF_CHUNK_SIZE);

  const visibleItems = $derived(items.slice(0, renderedCount));

  function updateArrows() {
    if (!scroller) return;
    canScrollLeft = scroller.scrollLeft > 8;
    canScrollRight = scroller.scrollLeft + scroller.clientWidth < scroller.scrollWidth - 8;
  }

  function scrollBy(direction: 1 | -1) {
    if (!scroller) return;
    const distance = Math.max(320, scroller.clientWidth * 0.8);
    scroller.scrollBy({ left: direction * distance, behavior: 'smooth' });
    // After a scroll-button click, the vanilla client appends more cards if the
    // user is near the end. Defer to after the smooth scroll settles.
    setTimeout(() => appendIfNeeded(), 220);
  }

  function appendIfNeeded() {
    if (!scroller) return;
    const remainingScroll = scroller.scrollWidth - (scroller.scrollLeft + scroller.clientWidth);
    const threshold = Math.max(360, scroller.clientWidth * 0.45);
    if (remainingScroll > threshold) return;
    if (renderedCount >= items.length) return;
    renderedCount = Math.min(items.length, renderedCount + HOME_SHELF_CHUNK_SIZE);
    // Re-evaluate arrows once new cards are in the DOM.
    queueMicrotask(updateArrows);
  }

  function onScroll() {
    updateArrows();
    appendIfNeeded();
  }

  // Reset rendered count when the item set changes (e.g. library switch).
  $effect(() => {
    // Read items length so the effect re-runs on changes.
    const total = items.length;
    renderedCount = Math.min(HOME_SHELF_CHUNK_SIZE, total);
    queueMicrotask(updateArrows);
  });
</script>

<section class="shelf" id={id}>
  <div class="shelf-header">
    <h3>{title}</h3>
    <span>{items.length} items</span>
  </div>
  <div class="shelf-row-shell" class:no-scroll={items.length <= HOME_SHELF_CHUNK_SIZE}>
    <button
      type="button"
      class="shelf-scroll-button"
      class:is-scroll-hidden={!canScrollLeft}
      title="Scroll left"
      onclick={() => scrollBy(-1)}
    >
      <Icon name="chevron-left" size={18} />
    </button>
    <div class="shelf-row" bind:this={scroller} onscroll={onScroll} data-shelf-row={rowCountId}>
      {#each visibleItems as card (card.id)}
        <MediaCard item={card} />
      {/each}
    </div>
    <button
      type="button"
      class="shelf-scroll-button"
      class:is-scroll-hidden={!canScrollRight}
      title="Scroll right"
      onclick={() => scrollBy(1)}
    >
      <Icon name="chevron-right" size={18} />
    </button>
  </div>
</section>
