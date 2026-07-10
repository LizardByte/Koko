// Declarative navigation regions — each major UI area declares itself as a
// region with entry points + internal navigation. The global engine finds
// the active region, calls its handler, and transitions between regions
// using declared entry points when the handler returns false (edge reached).
//
// This makes navigation predictable: "left from shelf" always goes to Home,
// not to whichever sidebar button is geometrically closest.

import type { Action } from 'svelte/action';
import type { Direction } from '$lib/gamepad';

export type RegionNavigateHandler = (
  direction: Direction,
  current: HTMLElement,
) => boolean; // true = handled, false = pass to global (region transition)

export type RegionEnterHandler = (
  direction: Direction,
) => HTMLElement | undefined;

export type RegionConfig = {
  name: string;
  /** Navigate within this region. Return true if handled, false to delegate. */
  navigate: RegionNavigateHandler;
  /** Element to focus when entering this region from a direction. */
  enter?: Partial<Record<Direction, RegionEnterHandler>>;
};

type RegisteredRegion = {
  name: string;
  container: HTMLElement;
  navigate: RegionNavigateHandler;
  enter: Partial<Record<Direction, RegionEnterHandler>>;
};

// --- Region registry ---

const regions: RegisteredRegion[] = [];

export function registerRegion(config: RegionConfig, container: HTMLElement): void {
  const idx = regions.findIndex((r) => r.container === container);
  if (idx >= 0) regions.splice(idx, 1);
  const region: RegisteredRegion = {
    name: config.name,
    container,
    navigate: config.navigate,
    enter: config.enter ?? {},
  };
  regions.push(region);
}

export function unregisterRegion(container: HTMLElement): void {
  const idx = regions.findIndex((r) => r.container === container);
  if (idx >= 0) regions.splice(idx, 1);
}

/** Find which region an element belongs to (walk up to nearest registered container). */
function findRegionForElement(el: HTMLElement): RegisteredRegion | undefined {
  let node: HTMLElement | null = el;
  while (node) {
    const region = regions.find((r) => r.container === node);
    if (region) return region;
    node = node.parentElement;
  }
  return undefined;
}

/** Find the nearest adjacent region in the given direction. */
function findAdjacentRegion(
  from: RegisteredRegion,
  direction: Direction,
): RegisteredRegion | undefined {
  const fromRect = from.container.getBoundingClientRect();
  const fcx = fromRect.left + fromRect.width / 2;
  const fcy = fromRect.top + fromRect.height / 2;

  let best: { region: RegisteredRegion; dist: number } | undefined;

  for (const region of regions) {
    if (region === from) continue;
    const rect = region.container.getBoundingClientRect();
    const dx = rect.left + rect.width / 2 - fcx;
    const dy = rect.top + rect.height / 2 - fcy;

    // Check the candidate is in the requested direction.
    switch (direction) {
      case 'right': if (dx < 5) continue; break;
      case 'left': if (dx > -5) continue; break;
      case 'down': if (dy < 5) continue; break;
      case 'up': if (dy > -5) continue; break;
    }

    const dist = Math.abs(dx) + Math.abs(dy);
    if (!best || dist < best.dist) best = { region, dist };
  }

  return best?.region;
}

// --- Global navigation entry point ---

/**
 * Handle a directional navigation request. Called by spatialNavigation
 * when the gamepad/keyboard fires a direction.
 *
 * Returns true if navigation was handled (focus moved or intentionally
 * stayed), false if nothing happened (no region, no target).
 */
const FOCUSABLE_QUERY = 'button:not(:disabled), a[href], [tabindex]:not([tabindex="-1"])';

/** Focus + scroll an element into view in one step. */
function focusAndScroll(el: HTMLElement | null | undefined): el is HTMLElement {
  if (!el) return false;
  el.focus();
  el.scrollIntoView({ block: 'nearest', inline: 'nearest' });
  return true;
}

/**
 * True when an element is actually visible (laid out + not display:none /
 * visibility:hidden / opacity:0). Uses Element.checkVisibility() — the
 * modern API that correctly handles position:fixed (which the older
 * offsetParent !== null check falsely reported as hidden). Deliberate
 * improvement over vanilla.
 */
function isVisible(el: HTMLElement): boolean {
  return el.checkVisibility({ checkOpacity: true, checkVisibilityCSS: true });
}

/**
 * First *visible* focusable descendant of a container.
 *
 * Uses isVisible() (checkVisibility) so position:fixed focusables aren't
 * skipped (unlike offsetParent) and hidden ones aren't returned (unlike
 * vanilla's bare querySelector). Deliberate improvement over vanilla.
 */
function firstFocusableIn(container: HTMLElement): HTMLElement | undefined {
  return Array.from(container.querySelectorAll<HTMLElement>(FOCUSABLE_QUERY)).find(
    (el) => el.checkVisibility({ checkOpacity: false, checkVisibilityCSS: true }),
  );
}

/** Handle the nothing-focused case: focus the first region's entry or first focusable. */
function focusInitial(direction: Direction): boolean {
  const first = regions[0];
  if (first?.enter?.[direction]) {
    const el = first.enter[direction]!(direction);
    if (focusAndScroll(el)) return true;
  }
  // Fallback: first focusable in any region.
  for (const r of regions) {
    const focusable = firstFocusableIn(r.container);
    if (focusable && focusAndScroll(focusable)) return true;
  }
  return false;
}

export function navigateDirection(direction: Direction): boolean {
  const current = document.activeElement as HTMLElement | null;
  if (!current) return focusInitial(direction);

  const region = findRegionForElement(current);
  if (!region) {
    // Outside all regions — fall back (spatial search will handle it).
    return false;
  }

  // Let the region try to handle it internally.
  if (region.navigate(direction, current)) return true;

  // Region couldn't handle it (edge reached) — transition to adjacent region.
  const adjacent = findAdjacentRegion(region, direction);
  if (adjacent) {
    // Use the adjacent region's entry handler for this direction.
    const target = adjacent.enter?.[direction]?.(direction);
    if (focusAndScroll(target)) return true;
    // No specific entry handler — focus the first focusable in the region.
    const focusable = firstFocusableIn(adjacent.container);
    if (focusable && focusAndScroll(focusable)) return true;
  }

  return false; // No adjacent region — stay.
}

// --- Helper: navigate a simple vertical/horizontal list ---

/**
 * Navigate within a container's focusable children in a list pattern.
 * Returns true if handled, false if at the edge (first/last element).
 */
export function navigateList(
  direction: Direction,
  current: HTMLElement,
  container: HTMLElement,
  options?: { horizontal?: boolean },
): boolean {
  const focusable = Array.from(
    container.querySelectorAll<HTMLElement>(
      'button:not(:disabled), a[href], [tabindex]:not([tabindex="-1"])',
    ),
  ).filter(isVisible);
  if (focusable.length === 0) return false;

  const idx = focusable.indexOf(current);
  if (idx === -1) {
    focusable[0]?.focus();
    return true;
  }

  const horizontal = options?.horizontal ?? false;
  const isForward = horizontal ? direction === 'right' : direction === 'down';
  const isBackward = horizontal ? direction === 'left' : direction === 'up';

  // Focus the neighbor at offset and return true; false if no neighbor.
  const focusNeighbor = (offset: number): boolean => {
    const next = focusable[idx + offset];
    if (!next) return false;
    next.focus();
    next.scrollIntoView({ block: 'nearest', inline: 'nearest' });
    return true;
  };

  if (isForward) return focusNeighbor(1);
  if (isBackward) return focusNeighbor(-1);
  return false; // Perpendicular direction — not a list navigation.
}

// --- Helper: navigate within a horizontal scroll row (shelf) ---

/**
 * Navigate left/right within a horizontally-scrolling row of cards.
 * Scrolls the row when reaching the edge before returning false.
 * Returns true if handled, false if at the true edge (after scroll).
 */
export function navigateShelfRow(
  direction: Direction,
  current: HTMLElement,
  row: HTMLElement,
): boolean {
  if (direction !== 'left' && direction !== 'right') return false;

  const cards = Array.from(
    row.querySelectorAll<HTMLElement>('.media-card:not(:disabled)'),
  ).filter(isVisible);
  if (cards.length === 0) return false;

  const idx = cards.indexOf(current);
  if (idx === -1) return false;

  /** Fully scroll a card into view within the row (both edges visible). */
  const scrollCardIntoView = (card: HTMLElement) => {
    const cardRect = card.getBoundingClientRect();
    const rowRect = row.getBoundingClientRect();
    if (cardRect.left < rowRect.left) {
      // Card is clipped on the left — scroll left to reveal it fully.
      row.scrollBy({ left: cardRect.left - rowRect.left - 8, behavior: 'smooth' });
    } else if (cardRect.right > rowRect.right) {
      // Card is clipped on the right — scroll right to reveal it fully.
      row.scrollBy({ left: cardRect.right - rowRect.right + 8, behavior: 'smooth' });
    }
  };

  if (direction === 'right') {
    if (idx < cards.length - 1) {
      const next = cards[idx + 1];
      next.focus();
      scrollCardIntoView(next);
      return true;
    }
    // At last card — try scrolling right to reveal more.
    if (row.scrollLeft + row.clientWidth < row.scrollWidth - 5) {
      row.scrollBy({ left: row.clientWidth * 0.8, behavior: 'smooth' });
      return true; // Stay — scrolled.
    }
    return false; // True edge.
  }

  if (direction === 'left') {
    if (idx > 0) {
      const prev = cards[idx - 1];
      prev.focus();
      scrollCardIntoView(prev);
      return true;
    }
    // At first card — try scrolling left.
    if (row.scrollLeft > 5) {
      row.scrollBy({ left: -row.clientWidth * 0.8, behavior: 'smooth' });
      return true; // Stay — scrolled.
    }
    return false; // True edge.
  }

  return false;
}

// --- Helper: find first visible card of first visible shelf ---

export function firstCardOfFirstShelf(container: HTMLElement): HTMLElement | undefined {
  const shelfRows = container.querySelectorAll<HTMLElement>('[data-shelf-row], .shelf-row');
  for (const row of shelfRows) {
    if (!isVisible(row)) continue;
    const card = row.querySelector<HTMLElement>('.media-card:not(:disabled)');
    if (card) return card;
  }
  // Fallback: any focusable card in the container.
  return container.querySelector<HTMLElement>('.media-card:not(:disabled)') ?? undefined;
}

// --- Svelte action ---

export const navRegion: Action<HTMLElement, RegionConfig> = (node, config) => {
  registerRegion(config, node);

  return {
    update(newConfig) {
      registerRegion(newConfig, node);
    },
    destroy() {
      unregisterRegion(node);
    },
  };
};
