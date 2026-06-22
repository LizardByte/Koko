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

/**
 * Read a single gamepad and normalize its input into a GamepadInput.
 * Pure function — no side effects, no DOM. Testable.
 */
export function readGamepad(gamepad: Gamepad, layout: GamepadLayout): GamepadInput {
  // D-pad
  let dpadUp = false, dpadDown = false, dpadLeft = false, dpadRight = false;

  if (layout.dpadButtons) {
    dpadUp = Boolean(gamepad.buttons[layout.dpadButtons.up]?.pressed);
    dpadDown = Boolean(gamepad.buttons[layout.dpadButtons.down]?.pressed);
    dpadLeft = Boolean(gamepad.buttons[layout.dpadButtons.left]?.pressed);
    dpadRight = Boolean(gamepad.buttons[layout.dpadButtons.right]?.pressed);
  } else if (layout.hatAxis !== undefined) {
    const hatValue = gamepad.axes[layout.hatAxis];
    if (hatValue !== undefined) {
      const hat = parseHat(hatValue, layout.hatNeutral ?? 0);
      dpadUp = hat.up;
      dpadDown = hat.down;
      dpadLeft = hat.left;
      dpadRight = hat.right;
    }
  }

  // Buttons
  const confirm = Boolean(gamepad.buttons[layout.confirm]?.pressed);
  const cancel = Boolean(gamepad.buttons[layout.cancel]?.pressed);
  const lBumper = Boolean(gamepad.buttons[layout.lBumper]?.pressed);
  const rBumper = Boolean(gamepad.buttons[layout.rBumper]?.pressed);
  const select = Boolean(gamepad.buttons[layout.select]?.pressed);
  const start = Boolean(gamepad.buttons[layout.start]?.pressed);

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

  // Raw buttons for extensibility
  const rawButtons = gamepad.buttons.map((b) => b.pressed);

  return {
    dpadUp, dpadDown, dpadLeft, dpadRight,
    confirm, cancel,
    lBumper, rBumper,
    select, start,
    leftStickX: lsx, leftStickY: lsy,
    rightStickX: rsx, rightStickY: rsy,
    rawButtons,
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
