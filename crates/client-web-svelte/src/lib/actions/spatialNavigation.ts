// Spatial navigation — gamepad + keyboard focus management for TV/couch use.
//
// Unlike vanilla's linear focus cycling (±1 in DOM order), this finds the
// nearest focusable element in the direction of input using geometric distance
// between bounding rects. D-pad up goes up, right goes right — matching the
// visual layout, not the DOM order.
//
// Usage: <div use:spatialNavigation>
// The action manages its own rAF polling loop + keyboard listener and cleans
// up on destroy. It reads `playback.isOpen` from the playback store to pause
// when the player overlay is active (the player has its own handler).

import type { Action } from 'svelte/action';
import { playback } from '$lib/stores';

type Direction = 'up' | 'down' | 'left' | 'right';

// Analog stick deadzone + hysteresis. The stick must exceed ACTIVATE to trigger
// a direction, then drop below RELEASE before it can trigger again. This
// prevents rapid-fire from stick drift or slightly-off centering.
const STICK_ACTIVATE = 0.4;
const STICK_RELEASE = 0.25;

// Minimum perpendicular overlap for a candidate to be considered "in direction."
// Prevents jumping to elements that are technically in the right direction but
// far off to the side.
const MIN_OVERLAP = 0.3; // fraction of the smaller element's dimension

const FOCUSABLE_SELECTOR =
  'button:not(:disabled), a[href], input:not(:disabled), select:not(:disabled), textarea:not(:disabled), [tabindex]:not([tabindex="-1"])';

function getVisibleFocusable(): HTMLElement[] {
  return Array.from(document.querySelectorAll<HTMLElement>(FOCUSABLE_SELECTOR)).filter(
    (el) => el.offsetParent !== null && el.getBoundingClientRect().width > 0,
  );
}

function getRect(el: HTMLElement): DOMRect {
  return el.getBoundingClientRect();
}

/**
 * Find the best element to focus in the given direction from `current`.
 * Uses spatial scoring: prefer elements that are closely aligned (small
 * perpendicular distance) and nearby (small parallel distance).
 */
function findSpatialTarget(current: HTMLElement, direction: Direction): HTMLElement | undefined {
  const focusable = getVisibleFocusable();
  if (focusable.length === 0) return undefined;

  const currentRect = getRect(current);
  const currentCenter = {
    x: currentRect.left + currentRect.width / 2,
    y: currentRect.top + currentRect.height / 2,
  };

  let best: { el: HTMLElement; score: number } | undefined;

  for (const candidate of focusable) {
    if (candidate === current) continue;

    const rect = getRect(candidate);
    const center = {
      x: rect.left + rect.width / 2,
      y: rect.top + rect.height / 2,
    };

    const dx = center.x - currentCenter.x;
    const dy = center.y - currentCenter.y;

    // Check the candidate is actually in the requested direction.
    let parallel: number; // distance in the movement direction
    let perpendicular: number; // distance perpendicular to movement

    switch (direction) {
      case 'right':
        if (dx < 2) continue; // must be to the right
        parallel = dx;
        perpendicular = Math.abs(dy);
        break;
      case 'left':
        if (dx > -2) continue; // must be to the left
        parallel = -dx;
        perpendicular = Math.abs(dy);
        break;
      case 'down':
        if (dy < 2) continue; // must be below
        parallel = dy;
        perpendicular = Math.abs(dx);
        break;
      case 'up':
        if (dy > -2) continue; // must be above
        parallel = -dy;
        perpendicular = Math.abs(dx);
        break;
    }

    // Reject candidates that are too far off-axis (not aligned with the direction).
    const minDim = Math.min(currentRect.width, currentRect.height, rect.width, rect.height);
    if (perpendicular > parallel * 2 && perpendicular > minDim * MIN_OVERLAP * 4) continue;

    // Score: lower is better. Weight perpendicular heavily (we want aligned
    // elements), then parallel (prefer closer).
    const score = perpendicular * 2 + parallel;

    if (!best || score < best.score) {
      best = { el: candidate, score };
    }
  }

  return best?.el;
}

function moveFocus(direction: Direction): void {
  const current = document.activeElement as HTMLElement | null;
  if (!current || !current.matches(FOCUSABLE_SELECTOR)) {
    // Nothing focused — focus the first visible element.
    const first = getVisibleFocusable()[0];
    first?.focus();
    first?.scrollIntoView({ block: 'nearest', behavior: 'smooth' });
    return;
  }

  const target = findSpatialTarget(current, direction);
  if (target) {
    target.focus();
    target.scrollIntoView({ block: 'nearest', behavior: 'smooth' });
    markSpatialFocus(target);
  }
}

function activateFocused(): void {
  const focused = document.activeElement as HTMLElement | null;
  if (focused?.matches('button, a[href], input, select, textarea, [role="button"]')) {
    focused.click();
  }
}

// --- Spatial focus indicator ---

let spatialFocusEl: HTMLElement | null = null;

function markSpatialFocus(el: HTMLElement) {
  if (spatialFocusEl) spatialFocusEl.classList.remove('is-spatial-focus');
  el.classList.add('is-spatial-focus');
  spatialFocusEl = el;
}

function clearSpatialFocus() {
  if (spatialFocusEl) {
    spatialFocusEl.classList.remove('is-spatial-focus');
    spatialFocusEl = null;
  }
}

// --- Gamepad state ---

const activeButtons = new Set<string>();

function pollGamepads() {
  const gamepads = navigator.getGamepads?.() ?? [];

  for (const gamepad of gamepads) {
    if (!gamepad) continue;

    // D-pad buttons (standard mapping: 12=up, 13=down, 14=left, 15=right)
    const dpadUp = Boolean(gamepad.buttons[12]?.pressed);
    const dpadDown = Boolean(gamepad.buttons[13]?.pressed);
    const dpadLeft = Boolean(gamepad.buttons[14]?.pressed);
    const dpadRight = Boolean(gamepad.buttons[15]?.pressed);
    const aButton = Boolean(gamepad.buttons[0]?.pressed);
    const bButton = Boolean(gamepad.buttons[1]?.pressed);

    // Analog stick (axes[0] = X, axes[1] = Y) with deadzone + hysteresis
    const stickX = gamepad.axes[0] ?? 0;
    const stickY = gamepad.axes[1] ?? 0;
    const stickActive = (name: string, value: number) =>
      activeButtons.has(`${gamepad.index}:${name}`)
        ? Math.abs(value) < STICK_RELEASE // already active — release check
        : Math.abs(value) > STICK_ACTIVATE; // not active — activate check

    const actions: Array<[string, boolean, () => void]> = [];

    // Only poll gamepad for spatial nav when player is NOT open
    if (!playback.isOpen) {
      actions.push(
        ['up', dpadUp || (stickY < 0 && stickActive('up', stickY)), () => moveFocus('up')],
        ['down', dpadDown || (stickY > 0 && stickActive('down', stickY)), () => moveFocus('down')],
        ['left', dpadLeft || (stickX < 0 && stickActive('left', stickX)), () => moveFocus('left')],
        ['right', dpadRight || (stickX > 0 && stickActive('right', stickX)), () => moveFocus('right')],
        ['activate', aButton, activateFocused],
        ['back', bButton, () => history.back()],
      );
    }

    for (const [name, pressed, action] of actions) {
      const key = `${gamepad.index}:${name}`;
      if (pressed && !activeButtons.has(key)) {
        activeButtons.add(key);
        action();
      } else if (!pressed) {
        activeButtons.delete(key);
      }
    }
  }

  rafId = requestAnimationFrame(pollGamepads);
}

let rafId = 0;

// --- Keyboard handler (same spatial logic for arrow keys) ---

function onKeydown(event: KeyboardEvent) {
  if (playback.isOpen) return; // player has its own handler
  if (event.defaultPrevented) return;

  const target = event.target as HTMLElement | null;
  if (target?.matches('input, textarea, select, [contenteditable="true"]')) return;

  switch (event.key) {
    case 'ArrowUp':
      event.preventDefault();
      moveFocus('up');
      break;
    case 'ArrowDown':
      event.preventDefault();
      moveFocus('down');
      break;
    case 'ArrowLeft':
      event.preventDefault();
      moveFocus('left');
      break;
    case 'ArrowRight':
      event.preventDefault();
      moveFocus('right');
      break;
    case 'Enter': {
      // Enter on a non-button focusable — activate it
      const focused = document.activeElement as HTMLElement | null;
      if (focused && !focused.matches('button, a[href]') && focused.matches(FOCUSABLE_SELECTOR)) {
        event.preventDefault();
        activateFocused();
      }
      break;
    }
  }
}

// --- Mouse detection (suppress spatial focus indicator when mouse is used) ---

function onMouseMove() {
  clearSpatialFocus();
}

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
