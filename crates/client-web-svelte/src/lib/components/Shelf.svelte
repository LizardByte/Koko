<script lang="ts">
  // A horizontal shelf/rail of media cards. Replaces the vanilla client's
  // renderRail()/renderShelf() plus the lazy-scroll + arrow-button logic in
  // eventBindings.ts (updateShelfScrollControls / refreshShelfScrollControls).
  // Here: native CSS scroll-snap + reactive arrow visibility via a scroll
  // listener. No virtualization yet (the vanilla client chunks via
  // data-lazy-shelf-id); that would be an IntersectionObserver addition for
  // large shelves.
  import MediaCard from './MediaCard.svelte';
  import Icon from './Icon.svelte';
  import type { MediaItemSummary } from '$lib/api';

  type Props = {
    title: string;
    items: MediaItemSummary[];
    id?: string;
  };
  let { title, items, id }: Props = $props();

  let scroller: HTMLDivElement | undefined = $state();
  let canScrollLeft = $state(false);
  let canScrollRight = $state(false);

  function updateArrows() {
    if (!scroller) return;
    canScrollLeft = scroller.scrollLeft > 8;
    canScrollRight = scroller.scrollLeft + scroller.clientWidth < scroller.scrollWidth - 8;
  }

  function scrollBy(direction: 1 | -1) {
    scroller?.scrollBy({ left: direction * scroller.clientWidth * 0.85, behavior: 'smooth' });
  }

  $effect(() => {
    // Re-evaluate arrow state when the item set changes.
    items;
    updateArrows();
  });
</script>

<section class="shelf" id={id} data-shelf-id={id}>
  <div class="shelf-header">
    <h2 class="shelf-title">{title}</h2>
    {#if canScrollLeft}
      <button class="scroll-btn left" onclick={() => scrollBy(-1)} aria-label="Scroll left">
        <Icon name="chevron-left" size={20} />
      </button>
    {/if}
    {#if canScrollRight}
      <button class="scroll-btn right" onclick={() => scrollBy(1)} aria-label="Scroll right">
        <Icon name="chevron-right" size={20} />
      </button>
    {/if}
  </div>
  <div
    class="shelf-track"
    bind:this={scroller}
    onscroll={updateArrows}
    role="list"
  >
    {#each items as item (item.id)}
      <div class="card-slot" role="listitem">
        <MediaCard {item} />
      </div>
    {/each}
  </div>
</section>

<style>
  .shelf {
    margin-bottom: 1.75rem;
  }
  .shelf-header {
    position: relative;
    display: flex;
    align-items: center;
    margin-bottom: 0.6rem;
  }
  .shelf-title {
    font-size: 1.05rem;
    font-weight: 600;
    margin: 0;
  }
  .scroll-btn {
    position: absolute;
    top: 50%;
    transform: translateY(-50%);
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 32px;
    height: 32px;
    border-radius: 50%;
    background: var(--koko-surface, #fff);
    border: 1px solid var(--koko-border, #ddd);
    box-shadow: 0 1px 4px rgba(0, 0, 0, 0.2);
    cursor: pointer;
    z-index: 2;
  }
  .scroll-btn:hover {
    background: var(--koko-border, #eee);
  }
  .scroll-btn.left {
    left: -16px;
  }
  .scroll-btn.right {
    right: -16px;
  }
  .shelf-track {
    display: flex;
    gap: 0.9rem;
    overflow-x: auto;
    scroll-snap-type: x proximity;
    padding: 4px 0 8px;
    scrollbar-width: thin;
  }
  .card-slot {
    scroll-snap-align: start;
  }
</style>
