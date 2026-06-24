// Spatial navigation — gamepad + keyboard focus management for TV/couch use.
//
// Uses the isolated gamepad package (readGamepad + detectLayout) for input
// normalization. The Koko-specific layer adds:
//   - Spatial focus navigation (direction-aware)
//   - D-pad: edge-triggered (fire once per press)
//   - Analog stick: directional debounce (fire once, discard for 1s, fire next)
//     NOT threshold/sensitivity — the raw direction is taken, then throttled.
//   - L/R bumpers = tab switching
//   - scrollIntoView (vertical + horizontal)
//   - Focus indicator (.is-spatial-focus)

import type { Action } from 'svelte/action';
import { playback, ui } from '$lib/stores';
import { readGamepad, detectLayout, applyDeadzone, type Direction } from '$lib/gamepad';
import { navigateDirection } from './navRegion';

const FOCUSABLE_SELECTOR =
  'button:not(:disabled), a[href], input:not(:disabled), select:not(:disabled), textarea:not(:disabled), [tabindex]:not([tabindex="-1"])';

// --- Spatial focus engine ---

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

function getVisibleFocusable(): HTMLElement[] {
  return Array.from(document.querySelectorAll<HTMLElement>(FOCUSABLE_SELECTOR)).filter(isVisible);
}

/**
 * Project a candidate's offset onto the requested direction, returning the
 * parallel (forward) and perpendicular (sideways) components. Returns null
 * when the candidate is behind the current element (negative parallel).
 */
function projectDirection(dx: number, dy: number, direction: Direction): { parallel: number; perpendicular: number } | null {
  switch (direction) {
    case 'right': return dx < 2 ? null : { parallel: dx, perpendicular: Math.abs(dy) };
    case 'left': return dx > -2 ? null : { parallel: -dx, perpendicular: Math.abs(dy) };
    case 'down': return dy < 2 ? null : { parallel: dy, perpendicular: Math.abs(dx) };
    case 'up': return dy > -2 ? null : { parallel: -dy, perpendicular: Math.abs(dx) };
  }
}

function findSpatialTarget(current: HTMLElement, direction: Direction): HTMLElement | undefined {
  const focusable = getVisibleFocusable();
  if (focusable.length === 0) return undefined;

  const currentRect = current.getBoundingClientRect();
  const cx = currentRect.left + currentRect.width / 2;
  const cy = currentRect.top + currentRect.height / 2;

  let best: { el: HTMLElement; score: number } | undefined;

  for (const candidate of focusable) {
    if (candidate === current) continue;
    const rect = candidate.getBoundingClientRect();
    const dx = rect.left + rect.width / 2 - cx;
    const dy = rect.top + rect.height / 2 - cy;

    const projected = projectDirection(dx, dy, direction);
    if (!projected) continue;
    const { parallel, perpendicular } = projected;

    const minDim = Math.min(currentRect.width, currentRect.height, rect.width, rect.height);
    if (perpendicular > parallel * 2 && perpendicular > minDim * 1.2) continue;

    const score = perpendicular * 2 + parallel;
    if (!best || score < best.score) best = { el: candidate, score };
  }
  return best?.el;
}

function moveFocus(direction: Direction): void {
  // Try the declarative region system first (predictable navigation).
  if (navigateDirection(direction)) {
    return; // Region handled it.
  }
  // Fallback: spatial search for elements outside any region.
  const current = document.activeElement as HTMLElement | null;
  if (!current || !current.matches(FOCUSABLE_SELECTOR)) {
    const first = getVisibleFocusable()[0];
    first?.focus();
    first?.scrollIntoView({ block: 'nearest', inline: 'nearest', behavior: 'smooth' });
    return;
  }
  const target = findSpatialTarget(current, direction);
  if (target) {
    target.focus();
    target.scrollIntoView({ block: 'nearest', inline: 'nearest', behavior: 'smooth' });
    markSpatialFocus(target);
  }
}

/** Relaxed spatial search — accepts any element vaguely in the direction. */


function activateFocused(): void {
  const el = document.activeElement as HTMLElement | null;
  if (el?.matches('button, a[href], input, select, textarea, [role="button"]')) {
    el.click();
  }
}

/** Wrap an index around [0, count) by step, cycling at the edges. */
function wrapTabIndex(index: number, count: number, step: number): number {
  const next = index + step;
  if (next < 0) return count - 1;
  if (next >= count) return 0;
  return next;
}

function switchTab(direction: 'left' | 'right'): void {
  // Context-aware tab switching. Find the tab group relevant to the current page.
  // Priority: browse tabs (home) > settings section nav > sidebar rail.
  const tabSelectors = [
    '.browse-tabs .browse-tab-button',      // Home browse tabs
    '.settings-section-nav button',          // Settings section nav
  ];

  for (const selector of tabSelectors) {
    const tabs = Array.from(document.querySelectorAll<HTMLElement>(selector))
      .filter(isVisible);
    if (tabs.length === 0) continue;

    const current = document.activeElement as HTMLElement | null;
    const idx = current ? tabs.indexOf(current) : -1;

    // If the focused element isn't in this tab group, find the active tab.
    let startIdx = idx;
    if (idx === -1) {
      const active = tabs.findIndex((t) => t.classList.contains('active'));
      startIdx = Math.max(active, 0);
    }

    const next = wrapTabIndex(startIdx, tabs.length, direction === 'left' ? -1 : 1);

    tabs[next]?.click();
    return; // First matching tab group wins.
  }
}

// --- Focus indicator ---

let spatialFocusEl: HTMLElement | null = null;
function markSpatialFocus(el: HTMLElement) {
  spatialFocusEl?.classList.remove('is-spatial-focus');
  el.classList.add('is-spatial-focus');
  spatialFocusEl = el;
}
function clearSpatialFocus() {
  spatialFocusEl?.classList.remove('is-spatial-focus');
  spatialFocusEl = null;
}

// --- Gamepad input handling ---
// D-pad: edge-triggered (fire once per physical press).
// Stick: directional debounce — take the dominant direction only (not magnitude).
//   Fire once on push, discard for INITIAL_DEBOUNCE_MS, then repeat every
//   REPEAT_MS. Reset immediately when the stick returns to center.

const STICK_INITIAL_DEBOUNCE_MS = 600; // First repeat after this long
const STICK_REPEAT_MS = 350;           // Subsequent repeats at this interval

let rafId = 0;
const pressedButtons = new Set<string>(); // Edge-trigger state for buttons
let lastStickFire = 0;       // Timestamp of last stick-triggered focus move
let lastStickDir: Direction | null = null; // Current held direction (null = released)
let stickHasFired = false;   // Has the initial fire happened for this push?

function pollGamepads() {
  const gamepads = navigator.getGamepads?.() ?? [];
  const now = performance.now();

  for (const gamepad of gamepads) {
    if (!gamepad) continue;
    const gid = gamepad.index;
    const layout = detectLayout(gamepad);
    const input = readGamepad(gamepad, layout);

    if (playback.isOpen) continue; // Player has its own handler

    // --- D-pad: edge-triggered ---
    edgeTrigger(`${gid}:dpad_up`, input.dpadUp, () => moveFocus('up'));
    edgeTrigger(`${gid}:dpad_down`, input.dpadDown, () => moveFocus('down'));
    edgeTrigger(`${gid}:dpad_left`, input.dpadLeft, () => moveFocus('left'));
    edgeTrigger(`${gid}:dpad_right`, input.dpadRight, () => moveFocus('right'));

    // --- Buttons: edge-triggered ---
    edgeTrigger(`${gid}:confirm`, input.confirm, activateFocused);
    edgeTrigger(`${gid}:cancel`, input.cancel, () => history.back());
    edgeTrigger(`${gid}:lBumper`, input.lBumper, () => switchTab('left'));
    edgeTrigger(`${gid}:rBumper`, input.rBumper, () => switchTab('right'));
    edgeTrigger(`${gid}:select`, input.select, () => ui.toggleControlsHelp());

    // --- Analog stick: directional debounce ---
    // Take the dominant direction only (not magnitude). Fire once on push,
    // discard for INITIAL_DEBOUNCE_MS, then repeat every REPEAT_MS.
    // Reset immediately when the stick returns to center.
    const [sx, sy] = applyDeadzone(input.leftStickX, input.leftStickY, 0.15);
    const dir = dominantDirection(sx, sy);
    if (dir) {
      if (dir !== lastStickDir) {
        // Direction changed or new push — fire immediately, start debounce
        lastStickDir = dir;
        lastStickFire = now;
        stickHasFired = true;
        moveFocus(dir);
      } else if (stickHasFired) {
        // Same direction still held — check debounce
        const elapsed = now - lastStickFire;
        // First repeat uses INITIAL_DEBOUNCE, subsequent use REPEAT
        const delay = elapsed < STICK_INITIAL_DEBOUNCE_MS
          ? STICK_INITIAL_DEBOUNCE_MS
          : STICK_REPEAT_MS;
        if (elapsed >= delay) {
          lastStickFire = now;
          moveFocus(dir);
        }
      }
    } else {
      // Stick released — reset immediately so next push fires right away
      lastStickDir = null;
      lastStickFire = 0;
      stickHasFired = false;
    }
  }

  rafId = requestAnimationFrame(pollGamepads);
}

/** Edge-trigger: fire action only on the rising edge (press), not while held. */
function edgeTrigger(key: string, pressed: boolean, action: () => void) {
  if (pressed && !pressedButtons.has(key)) {
    pressedButtons.add(key);
    action();
  } else if (!pressed) {
    pressedButtons.delete(key);
  }
}

/** Extract the dominant direction from stick values (>0.15 deflection). */
function dominantDirection(x: number, y: number): Direction | null {
  if (Math.abs(x) < 0.15 && Math.abs(y) < 0.15) return null;
  if (Math.abs(x) > Math.abs(y)) {
    return x > 0 ? 'right' : 'left';
  }
  return y > 0 ? 'down' : 'up';
}

// --- Keyboard handler ---

const ARROW_KEY_DIRECTIONS: Record<string, Direction> = {
  ArrowUp: 'up',
  ArrowDown: 'down',
  ArrowLeft: 'left',
  ArrowRight: 'right',
};

function onKeydown(event: KeyboardEvent) {
  if (playback.isOpen) return;
  if (event.defaultPrevented) return;
  const target = event.target as HTMLElement | null;
  if (target?.matches('input, textarea, select, [contenteditable="true"]')) return;

  const arrowDir = ARROW_KEY_DIRECTIONS[event.key];
  if (arrowDir) {
    event.preventDefault();
    moveFocus(arrowDir);
    return;
  }

  switch (event.key) {
    case 'Enter': {
      const focused = document.activeElement as HTMLElement | null;
      if (focused && !focused.matches('button, a[href]') && focused.matches(FOCUSABLE_SELECTOR)) {
        event.preventDefault();
        activateFocused();
      }
      break;
    }
    case '?': event.preventDefault(); ui.toggleControlsHelp(); break;
    case '[': switchTab('left'); break;
    case ']': switchTab('right'); break;
  }
}

function onMouseMove() { clearSpatialFocus(); }

// --- Svelte action ---

export const spatialNavigation: Action<HTMLElement> = (node) => {
  node.addEventListener('keydown', onKeydown);
  globalThis.addEventListener('mousemove', onMouseMove, { passive: true });
  rafId = requestAnimationFrame(pollGamepads);

  return {
    destroy() {
      node.removeEventListener('keydown', onKeydown);
      globalThis.removeEventListener('mousemove', onMouseMove);
      cancelAnimationFrame(rafId);
      clearSpatialFocus();
    },
  };
};
