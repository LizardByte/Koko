import type { AppState } from './types';

const activeGamepadButtons = new Set<string>();

function visibleFocusableElements(): HTMLElement[] {
  return Array.from(
    document.querySelectorAll<HTMLElement>(
      'button:not(:disabled), a[href], input:not(:disabled), select:not(:disabled), textarea:not(:disabled), [tabindex]:not([tabindex="-1"])',
    ),
  ).filter((element) => element.offsetParent !== null);
}

function moveFocus(direction: 1 | -1): void {
  const focusable = visibleFocusableElements();
  if (!focusable.length) {
    return;
  }
  const currentIndex = Math.max(0, focusable.indexOf(document.activeElement as HTMLElement));
  focusable[(currentIndex + direction + focusable.length) % focusable.length]?.focus();
}

function activateFocusedElement(): void {
  const focused = document.activeElement as HTMLElement | null;
  if (focused?.matches('button, a[href], input, select, textarea')) {
    focused.click();
  }
}

function pollGamepads(): void {
  const gamepads = navigator.getGamepads?.() ?? [];
  gamepads.forEach((gamepad) => {
    if (!gamepad) {
      return;
    }
    const actions: Array<[string, boolean, () => void]> = [
      ['up', Boolean(gamepad.buttons[12]?.pressed) || gamepad.axes[1] < -0.65, () => moveFocus(-1)],
      ['down', Boolean(gamepad.buttons[13]?.pressed) || gamepad.axes[1] > 0.65, () => moveFocus(1)],
      ['left', Boolean(gamepad.buttons[14]?.pressed) || gamepad.axes[0] < -0.65, () => moveFocus(-1)],
      ['right', Boolean(gamepad.buttons[15]?.pressed) || gamepad.axes[0] > 0.65, () => moveFocus(1)],
      ['activate', Boolean(gamepad.buttons[0]?.pressed), activateFocusedElement],
      ['back', Boolean(gamepad.buttons[1]?.pressed), () => window.history.back()],
    ];
    actions.forEach(([name, pressed, action]) => {
      const key = `${gamepad.index}:${name}`;
      if (pressed && !activeGamepadButtons.has(key)) {
        activeGamepadButtons.add(key);
        action();
      } else if (!pressed) {
        activeGamepadButtons.delete(key);
      }
    });
  });
  window.requestAnimationFrame(pollGamepads);
}

/** Binds global keyboard and gamepad navigation controls for non-player screens. */
export function bindGlobalInputHandlers(state: Pick<AppState, 'activeTrailer' | 'isPlayerOpen'>): void {
  window.addEventListener('keydown', (event) => {
    if (state.isPlayerOpen || state.activeTrailer || event.defaultPrevented) {
      return;
    }
    if (!['ArrowRight', 'ArrowLeft', 'ArrowDown', 'ArrowUp'].includes(event.key)) {
      return;
    }
    const target = event.target as HTMLElement | null;
    if (target?.matches('input, textarea, select, [contenteditable="true"]')) {
      return;
    }
    const focusable = visibleFocusableElements();
    if (!focusable.length) {
      return;
    }
    const currentIndex = Math.max(0, focusable.indexOf(document.activeElement as HTMLElement));
    const direction = event.key === 'ArrowRight' || event.key === 'ArrowDown' ? 1 : -1;
    focusable[(currentIndex + direction + focusable.length) % focusable.length]?.focus();
    event.preventDefault();
  });

  window.requestAnimationFrame(pollGamepads);
}
