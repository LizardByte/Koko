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

type Direction = 'up' | 'down' | 'left' | 'right';

// Analog stick thresholds — must exceed ACTIVATE to trigger, then drop below
// RELEASE before re-triggering. The 8BitDo Pro 3 has a long throw, so we use
// a high threshold to prevent accidental triggers.
const STICK_ACTIVATE = 0.7;
const STICK_RELEASE = 0.35;

// Repeat delay for held directional input (ms). After the initial trigger,
// holding a direction re-triggers after this delay. Prevents rapid-fire.
const REPEAT_DELAY = 250;
const REPEAT_INITIAL_DELAY = 400;

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

    // --- D-pad detection ---
    // Try standard buttons first (12-15), then hat axis (8BitDo).
    const dpadUp = Boolean(gamepad.buttons[12]?.pressed);
    const dpadDown = Boolean(gamepad.buttons[13]?.pressed);
    const dpadLeft = Boolean(gamepad.buttons[14]?.pressed);
    const dpadRight = Boolean(gamepad.buttons[15]?.pressed);

    // Hat axis fallback: check if axes 9 (or 8) behaves like a D-pad hat.
    // The 8BitDo Pro 3 reports D-pad on axis 9 with values like:
    // up=-1, upleft=-0.71, left=-0.43, downleft=0.14, down=0.43...
    // We use thresholds to detect 4 cardinal directions.
    let hatUp = false, hatDown = false, hatLeft = false, hatRight = false;
    const hatValue = gamepad.axes[9];
    if (hatValue !== undefined && Math.abs(hatValue) > 0.05) {
      // Neutral is typically 0 or a specific value. Map the 8-position hat.
      if (hatValue < -0.85 || (hatValue > 0.85 && hatValue <= 1)) hatUp = true;
      if (hatValue > 0.28 && hatValue < 0.57) hatDown = true;
      if (hatValue > -0.57 && hatValue < -0.28) hatLeft = true;
      if (hatValue > 0.05 && hatValue < 0.28) hatRight = true;
      // Handle the wrap-around case (max was 3.28 in probe)
      if (hatValue > 1.5) hatUp = true; // wrapped up
    }

    const up = dpadUp || hatUp;
    const down = dpadDown || hatDown;
    const left = dpadLeft || hatLeft;
    const right = dpadRight || hatRight;

    // Face buttons: A=0, B=1 (may differ on non-standard mapping)
    const aButton = Boolean(gamepad.buttons[0]?.pressed);
    const bButton = Boolean(gamepad.buttons[1]?.pressed);
    // L/R bumpers: 4=L, 5=R (standard); fallback 6/7
    const lButton = Boolean(gamepad.buttons[4]?.pressed || gamepad.buttons[6]?.pressed);
    const rButton = Boolean(gamepad.buttons[5]?.pressed || gamepad.buttons[7]?.pressed);
    // Select/Back = 8, Start = 9, Guide/Home = 16
    const selectButton = Boolean(gamepad.buttons[8]?.pressed || gamepad.buttons[16]?.pressed);

    // Left stick (axes 0=X, 1=Y) — spatial navigation
    const leftX = gamepad.axes[0] ?? 0;
    const leftY = gamepad.axes[1] ?? 0;

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
