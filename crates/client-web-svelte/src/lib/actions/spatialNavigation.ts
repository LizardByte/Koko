// Spatial navigation — gamepad + keyboard focus management for TV/couch use.
//
// Supports both standard-mapping gamepads (D-pad = buttons 12-15) AND raw/
// non-standard gamepads like the 8BitDo Pro 3 (D-pad = hat on axis 9, no
// standard mapping). Auto-detects the D-pad location per connected gamepad.
//
// Improvements over vanilla:
//   - Spatial focus (direction-aware, not ±1 in DOM order)
//   - Analog stick: high threshold + repeat-delay (not edge-triggered)
//   - D-pad auto-detection (hat axis vs buttons)
//   - Right stick = volume in player, page scroll in browse
//   - L/R bumpers = tab switching
//   - scrollIntoView (vertical + horizontal)
//   - Focus indicator (.is-spatial-focus)

import type { Action } from 'svelte/action';
import { playback, ui } from '$lib/stores';
import { detectLayout } from '$lib/gamepad';

type Direction = 'up' | 'down' | 'left' | 'right';

// Analog stick thresholds — must exceed ACTIVATE to trigger, then drop below
// RELEASE before re-triggering. The 8BitDo Pro 3 has a long throw, so we use
// a high threshold to prevent accidental triggers.
const STICK_ACTIVATE = 0.7;
const STICK_RELEASE = 0.35;

// Repeat delay for held directional input (ms). After the initial trigger,
// holding a direction re-triggers after this delay. These are deliberately
// slow so the user can see each focus step.
const REPEAT_DELAY = 350;
const REPEAT_INITIAL_DELAY = 500;

const FOCUSABLE_SELECTOR =
  'button:not(:disabled), a[href], input:not(:disabled), select:not(:disabled), textarea:not(:disabled), [tabindex]:not([tabindex="-1"])';

function getVisibleFocusable(): HTMLElement[] {
  return Array.from(document.querySelectorAll<HTMLElement>(FOCUSABLE_SELECTOR)).filter(
    (el) => el.offsetParent !== null && el.getBoundingClientRect().width > 0,
  );
}

function findSpatialTarget(current: HTMLElement, direction: Direction): HTMLElement | undefined {
  const focusable = getVisibleFocusable();
  if (focusable.length === 0) return undefined;

  const currentRect = current.getBoundingClientRect();
  const currentCenter = {
    x: currentRect.left + currentRect.width / 2,
    y: currentRect.top + currentRect.height / 2,
  };

  let best: { el: HTMLElement; score: number } | undefined;

  for (const candidate of focusable) {
    if (candidate === current) continue;

    const rect = candidate.getBoundingClientRect();
    const center = {
      x: rect.left + rect.width / 2,
      y: rect.top + rect.height / 2,
    };

    const dx = center.x - currentCenter.x;
    const dy = center.y - currentCenter.y;

    let parallel: number;
    let perpendicular: number;

    switch (direction) {
      case 'right':
        if (dx < 2) continue;
        parallel = dx;
        perpendicular = Math.abs(dy);
        break;
      case 'left':
        if (dx > -2) continue;
        parallel = -dx;
        perpendicular = Math.abs(dy);
        break;
      case 'down':
        if (dy < 2) continue;
        parallel = dy;
        perpendicular = Math.abs(dx);
        break;
      case 'up':
        if (dy > -2) continue;
        parallel = -dy;
        perpendicular = Math.abs(dx);
        break;
    }

    const minDim = Math.min(currentRect.width, currentRect.height, rect.width, rect.height);
    if (perpendicular > parallel * 2 && perpendicular > minDim * 1.2) continue;

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

function activateFocused(): void {
  const focused = document.activeElement as HTMLElement | null;
  if (focused?.matches('button, a[href], input, select, textarea, [role="button"]')) {
    focused.click();
  }
}

// --- Tab switching (L/R bumpers) ---

function switchTab(direction: 'left' | 'right'): void {
  // Find all tab-like buttons in the current page (home tabs, settings nav, etc.)
  const tabs = Array.from(document.querySelectorAll<HTMLElement>(
    '.library-rail-top .rail-button, .settings-section-nav button, .home-tab',
  )).filter((el) => el.offsetParent !== null);
  if (tabs.length === 0) return;

  const current = document.activeElement as HTMLElement | null;
  const currentIndex = current ? tabs.indexOf(current) : -1;

  let nextIndex: number;
  if (direction === 'left') {
    nextIndex = currentIndex > 0 ? currentIndex - 1 : tabs.length - 1;
  } else {
    nextIndex = currentIndex < tabs.length - 1 ? currentIndex + 1 : 0;
  }

  tabs[nextIndex]?.click();
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

const activeInputs = new Set<string>();
// Repeat timing: track when each directional input last fired.
const lastFireTime: Record<string, number> = {};

function pollGamepads() {
  const gamepads = navigator.getGamepads?.() ?? [];
  const now = performance.now();

  for (const gamepad of gamepads) {
    if (!gamepad) continue;
    const gid = gamepad.index;
    const layout = detectLayout(gamepad);

    // --- D-pad detection ---
    // Standard layout: D-pad as buttons. 8BitDo etc.: D-pad as hat axis.
    let up: boolean, down: boolean, left: boolean, right: boolean;

    if (layout.dpadButtons) {
      up = Boolean(gamepad.buttons[layout.dpadButtons.up]?.pressed);
      down = Boolean(gamepad.buttons[layout.dpadButtons.down]?.pressed);
      left = Boolean(gamepad.buttons[layout.dpadButtons.left]?.pressed);
      right = Boolean(gamepad.buttons[layout.dpadButtons.right]?.pressed);
    } else {
      // Hat axis (e.g., 8BitDo Pro 3 axis 9). 8-position hat.
      // The neutral value varies per controller (0 for most, 3.286 for 8BitDo).
      up = down = left = right = false;
      const hatAxis = layout.hatAxis ?? 9;
      const hatNeutral = layout.hatNeutral ?? 0;
      const hatValue = gamepad.axes[hatAxis];
      if (hatValue !== undefined && Math.abs(hatValue - hatNeutral) > 0.2) {
        if (hatValue <= -0.85) up = true;
        if (hatValue <= -0.6 && hatValue > -0.85) { up = true; right = true; }
        if (hatValue <= -0.3 && hatValue > -0.6) right = true;
        if (hatValue <= -0.1 && hatValue > -0.3) { down = true; right = true; }
        if (hatValue >= 0.1 && hatValue < 0.3) down = true;
        if (hatValue >= 0.3 && hatValue < 0.6) { down = true; left = true; }
        if (hatValue >= 0.6 && hatValue < 0.85) left = true;
        if (hatValue >= 0.85 && hatValue <= 1.1) { up = true; left = true; }
      }
    }

    // Face buttons — layout-dependent (Nintendo vs Xbox layout).
    const aButton = Boolean(gamepad.buttons[layout.confirm]?.pressed);
    const bButton = Boolean(gamepad.buttons[layout.cancel]?.pressed);
    const lButton = Boolean(gamepad.buttons[layout.lBumper]?.pressed);
    const rButton = Boolean(gamepad.buttons[layout.rBumper]?.pressed);
    const selectButton = Boolean(gamepad.buttons[layout.select]?.pressed);

    // Left stick — layout-dependent axes.
    const leftX = gamepad.axes[layout.leftStick[0]] ?? 0;
    const leftY = gamepad.axes[layout.leftStick[1]] ?? 0;

    const stickActive = (name: string, value: number) => {
      const key = `${gid}:${name}`;
      if (activeInputs.has(key)) {
        // Already active — check release
        return Math.abs(value) < STICK_RELEASE;
      }
      // Not active — check activate
      return Math.abs(value) > STICK_ACTIVATE;
    };

    // Helper for edge-triggered buttons (fire once per press)
    const edgeTrigger = (name: string, pressed: boolean, action: () => void) => {
      const key = `${gid}:${name}`;
      if (pressed && !activeInputs.has(key)) {
        activeInputs.add(key);
        action();
      } else if (!pressed) {
        activeInputs.delete(key);
      }
    };

    // Helper for directional input with repeat delay
    const directionalTrigger = (name: string, pressed: boolean, action: () => void) => {
      const key = `${gid}:${name}`;
      if (!pressed) {
        activeInputs.delete(key);
        delete lastFireTime[key];
        return;
      }
      // Pressed
      const lastFire = lastFireTime[key] ?? 0;
      const delay = key in lastFireTime ? REPEAT_DELAY : REPEAT_INITIAL_DELAY;
      if (!activeInputs.has(key)) {
        // First press
        activeInputs.add(key);
        lastFireTime[key] = now;
        action();
      } else if (now - lastFire >= delay) {
        // Repeat
        lastFireTime[key] = now;
        action();
      }
    };

    if (!playback.isOpen) {
      // Browse mode
      directionalTrigger('nav_up', up || (leftY < 0 && stickActive('nav_up', leftY)), () => moveFocus('up'));
      directionalTrigger('nav_down', down || (leftY > 0 && stickActive('nav_down', leftY)), () => moveFocus('down'));
      directionalTrigger('nav_left', left || (leftX < 0 && stickActive('nav_left', leftX)), () => moveFocus('left'));
      directionalTrigger('nav_right', right || (leftX > 0 && stickActive('nav_right', leftX)), () => moveFocus('right'));
      edgeTrigger('activate', aButton, activateFocused);
      edgeTrigger('back', bButton, () => history.back());
      edgeTrigger('tab_left', lButton, () => switchTab('left'));
      edgeTrigger('tab_right', rButton, () => switchTab('right'));
      edgeTrigger('help', selectButton, () => ui.toggleControlsHelp());
    }
  }

  rafId = requestAnimationFrame(pollGamepads);
}

let rafId = 0;

// --- Keyboard handler ---

function onKeydown(event: KeyboardEvent) {
  if (playback.isOpen) return;
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
      const focused = document.activeElement as HTMLElement | null;
      if (focused && !focused.matches('button, a[href]') && focused.matches(FOCUSABLE_SELECTOR)) {
        event.preventDefault();
        activateFocused();
      }
      break;
    }
    case '?':
      event.preventDefault();
      ui.toggleControlsHelp();
      break;
    case '[':
      switchTab('left');
      break;
    case ']':
      switchTab('right');
      break;
  }
}

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
