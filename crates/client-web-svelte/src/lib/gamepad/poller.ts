// Gamepad poller — the core rAF polling engine. Framework-agnostic.
//
// Reads connected gamepads every frame, applies the detected layout,
// normalizes all inputs into a GamepadInput object, and calls a callback.
// The caller decides what to do with the input (spatial navigation, player
// controls, etc.).

import { detectLayout } from './layouts';
import { parseHat } from './hat';
import { applyDeadzone } from './stick';
import type { GamepadInput, GamepadLayout } from './types';

export type InputCallback = (input: GamepadInput, gamepad: Gamepad, layout: GamepadLayout) => void;

const DEADZONE = 0.12;

/** Read a button state from a (possibly out-of-range) index. */
function btn(gamepad: Gamepad, index: number): boolean {
  return Boolean(gamepad.buttons[index]?.pressed);
}

/** Read the D-pad state from buttons or hat axis (layout-aware). */
function readDpad(gamepad: Gamepad, layout: GamepadLayout): { up: boolean; down: boolean; left: boolean; right: boolean } {
  if (layout.dpadButtons) {
    return {
      up: btn(gamepad, layout.dpadButtons.up),
      down: btn(gamepad, layout.dpadButtons.down),
      left: btn(gamepad, layout.dpadButtons.left),
      right: btn(gamepad, layout.dpadButtons.right),
    };
  }
  if (layout.hatAxis !== undefined) {
    const hatValue = gamepad.axes[layout.hatAxis];
    if (hatValue !== undefined) {
      return parseHat(hatValue, layout.hatNeutral ?? 0);
    }
  }
  return { up: false, down: false, left: false, right: false };
}

/**
 * Read a single gamepad and normalize its input into a GamepadInput.
 * Pure function — no side effects, no DOM. Testable.
 */
export function readGamepad(gamepad: Gamepad, layout: GamepadLayout): GamepadInput {
  const dpad = readDpad(gamepad, layout);

  // Sticks
  const [lsx, lsy] = applyDeadzone(
    gamepad.axes[layout.leftStick[0]] ?? 0,
    gamepad.axes[layout.leftStick[1]] ?? 0,
    DEADZONE,
  );
  const [rsx, rsy] = applyDeadzone(
    gamepad.axes[layout.rightStick[0]] ?? 0,
    gamepad.axes[layout.rightStick[1]] ?? 0,
    DEADZONE,
  );

  return {
    dpadUp: dpad.up, dpadDown: dpad.down, dpadLeft: dpad.left, dpadRight: dpad.right,
    confirm: btn(gamepad, layout.confirm),
    cancel: btn(gamepad, layout.cancel),
    lBumper: btn(gamepad, layout.lBumper),
    rBumper: btn(gamepad, layout.rBumper),
    select: btn(gamepad, layout.select),
    start: btn(gamepad, layout.start),
    leftStickX: lsx, leftStickY: lsy,
    rightStickX: rsx, rightStickY: rsy,
    rawButtons: gamepad.buttons.map((b) => b.pressed),
  };
}

/**
 * Start a rAF polling loop that calls `callback` for each connected gamepad
 * every frame. Returns a cleanup function that stops the loop.
 *
 * The callback receives the normalized GamepadInput + the detected layout,
 * so the caller can apply edge-triggering / repeat-delay / hysteresis logic
 * specific to their use case.
 */
export function startPolling(callback: InputCallback): () => void {
  let rafId = 0;
  // Cache layouts per gamepad index (avoid re-detecting every frame).
  const layoutCache = new Map<number, GamepadLayout>();

  function tick() {
    const gamepads = navigator.getGamepads?.() ?? [];
    for (const gamepad of gamepads) {
      if (!gamepad) continue;

      let layout = layoutCache.get(gamepad.index);
      if (!layout) {
        layout = detectLayout(gamepad);
        layoutCache.set(gamepad.index, layout);
      }

      const input = readGamepad(gamepad, layout);
      callback(input, gamepad, layout);
    }
    rafId = requestAnimationFrame(tick);
  }

  rafId = requestAnimationFrame(tick);

  return () => {
    cancelAnimationFrame(rafId);
    layoutCache.clear();
  };
}
