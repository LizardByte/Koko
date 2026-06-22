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

const FOCUSABLE_SELECTOR =
  'button:not(:disabled), a[href], input:not(:disabled), select:not(:disabled), textarea:not(:disabled), [tabindex]:not([tabindex="-1"])';

// --- Spatial focus engine ---

function getVisibleFocusable(): HTMLElement[] {
  return Array.from(document.querySelectorAll<HTMLElement>(FOCUSABLE_SELECTOR)).filter(
    (el) => el.offsetParent !== null && el.getBoundingClientRect().width > 0,
  );
}

function findSpatialTarget(current: HTMLElement, direction: Direction): HTMLElement | undefined {
  const focusable = getVisibleFocusable();
  if (focusable.length === 0) return undefined;

  const currentRect = current.getBoundingClientRect();
  const cx = currentRect.left + currentRect.width / 2;
  const cy = currentRect.top + currentRect.height / 2;

  // Find the scroll container of the current element — we want to prefer
  // staying within the same container when possible (e.g., scrolling through
  // a shelf before jumping to another section).
  const currentScrollParent = getScrollParent(current);

  let best: { el: HTMLElement; score: number; sameContainer: boolean } | undefined;

  for (const candidate of focusable) {
    if (candidate === current) continue;
    const rect = candidate.getBoundingClientRect();
    const dx = rect.left + rect.width / 2 - cx;
    const dy = rect.top + rect.height / 2 - cy;

    let parallel: number;
    let perpendicular: number;
    switch (direction) {
      case 'right': if (dx < 2) continue; parallel = dx; perpendicular = Math.abs(dy); break;
      case 'left': if (dx > -2) continue; parallel = -dx; perpendicular = Math.abs(dy); break;
      case 'down': if (dy < 2) continue; parallel = dy; perpendicular = Math.abs(dx); break;
      case 'up': if (dy > -2) continue; parallel = -dy; perpendicular = Math.abs(dx); break;
    }

    // Determine if this candidate is in the same scroll container.
    const candidateScrollParent = getScrollParent(candidate);
    const sameContainer = currentScrollParent !== null && currentScrollParent === candidateScrollParent;

    // If looking in the movement direction within the same container, prefer
    // same-container elements (stay in the shelf, don't jump out prematurely).
    // For perpendicular/escape directions (leaving a container), don't penalize
    // cross-container elements.
    let score = perpendicular * 2 + parallel;
    if (sameContainer) score *= 0.5; // Boost same-container candidates

    const minDim = Math.min(currentRect.width, currentRect.height, rect.width, rect.height);
    if (perpendicular > parallel * 2 && perpendicular > minDim * 1.2) continue;

    if (!best || score < best.score) {
      best = { el: candidate, score, sameContainer };
    }
  }

  return best?.el;
}

/** Find the nearest scrollable ancestor of an element. */
function getScrollParent(el: HTMLElement): HTMLElement | null {
  let parent = el.parentElement;
  while (parent) {
    const style = getComputedStyle(parent);
    if (
      (style.overflowY === 'auto' || style.overflowY === 'scroll' ||
       style.overflowX === 'auto' || style.overflowX === 'scroll') &&
      (parent.scrollHeight > parent.clientHeight || parent.scrollWidth > parent.clientWidth)
    ) {
      return parent;
    }
    parent = parent.parentElement;
  }
  return null;
}

function moveFocus(direction: Direction): void {
  const current = document.activeElement as HTMLElement | null;
  if (!current || !current.matches(FOCUSABLE_SELECTOR)) {
    const first = getVisibleFocusable()[0];
    first?.focus();
    first?.scrollIntoView({ block: 'nearest', inline: 'nearest', behavior: 'smooth' });
    return;
  }

  let target = findSpatialTarget(current, direction);

  // Fallback: if no target found (e.g., trapped in sidebar or at end of
  // shelf), try with relaxed constraints — accept any element in the
  // general direction, even if far away.
  if (!target) {
    target = findSpatialTargetRelaxed(current, direction);
  }

  if (target) {
    target.focus();
    target.scrollIntoView({ block: 'nearest', inline: 'nearest', behavior: 'smooth' });
    markSpatialFocus(target);
  }
}

/** Relaxed spatial search — accepts any element vaguely in the direction. */
function findSpatialTargetRelaxed(current: HTMLElement, direction: Direction): HTMLElement | undefined {
  const focusable = getVisibleFocusable();
  const currentRect = current.getBoundingClientRect();
  const cx = currentRect.left + currentRect.width / 2;
  const cy = currentRect.top + currentRect.height / 2;

  let best: { el: HTMLElement; dist: number } | undefined;

  for (const candidate of focusable) {
    if (candidate === current) continue;
    const rect = candidate.getBoundingClientRect();
    const dx = rect.left + rect.width / 2 - cx;
    const dy = rect.top + rect.height / 2 - cy;

    // Just check the sign of the movement direction — no perpendicular filter.
    let dist: number;
    switch (direction) {
      case 'right': if (dx < 0) continue; dist = Math.abs(dx) + Math.abs(dy) * 0.5; break;
      case 'left': if (dx > 0) continue; dist = Math.abs(dx) + Math.abs(dy) * 0.5; break;
      case 'down': if (dy < 0) continue; dist = Math.abs(dy) + Math.abs(dx) * 0.5; break;
      case 'up': if (dy > 0) continue; dist = Math.abs(dy) + Math.abs(dx) * 0.5; break;
    }

    if (!best || dist < best.dist) {
      best = { el: candidate, dist };
    }
  }
  return best?.el;
}

function activateFocused(): void {
  const el = document.activeElement as HTMLElement | null;
  el?.matches('button, a[href], input, select, textarea, [role="button"]') && el.click();
}

function switchTab(direction: 'left' | 'right'): void {
  const tabs = Array.from(document.querySelectorAll<HTMLElement>(
    '.library-rail-top .rail-button, .settings-section-nav button, .home-tab',
  )).filter((el) => el.offsetParent !== null);
  if (tabs.length === 0) return;
  const current = document.activeElement as HTMLElement | null;
  const idx = current ? tabs.indexOf(current) : -1;
  const next = direction === 'left' ? (idx > 0 ? idx - 1 : tabs.length - 1) : (idx < tabs.length - 1 ? idx + 1 : 0);
  tabs[next]?.click();
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
  return Math.abs(x) > Math.abs(y)
    ? (x > 0 ? 'right' : 'left')
    : (y > 0 ? 'down' : 'up');
}

// --- Keyboard handler ---

function onKeydown(event: KeyboardEvent) {
  if (playback.isOpen) return;
  if (event.defaultPrevented) return;
  const target = event.target as HTMLElement | null;
  if (target?.matches('input, textarea, select, [contenteditable="true"]')) return;

  switch (event.key) {
    case 'ArrowUp': event.preventDefault(); moveFocus('up'); break;
    case 'ArrowDown': event.preventDefault(); moveFocus('down'); break;
    case 'ArrowLeft': event.preventDefault(); moveFocus('left'); break;
    case 'ArrowRight': event.preventDefault(); moveFocus('right'); break;
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
  window.addEventListener('mousemove', onMouseMove, { passive: true });
  rafId = requestAnimationFrame(pollGamepads);

  return {
    destroy() {
      node.removeEventListener('keydown', onKeydown);
      window.removeEventListener('mousemove', onMouseMove);
      cancelAnimationFrame(rafId);
      clearSpatialFocus();
    },
  };
};
