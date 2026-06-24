// Player keyboard + gamepad shortcuts action.
// Usage: <div use:playerShortcuts={{ onPlayPause, onSeek, onMute, onFullscreen, onClose }}>
//
// Keyboard: Space/k (play/pause), arrows (seek/volume), m (mute), f (fullscreen), Escape (close)
// Gamepad: A=play/pause, B=close, left/right=seek, up/down=volume (layout-aware)

import type { Action } from 'svelte/action';
import { detectLayout, type GamepadLayout } from '$lib/gamepad';

export type PlayerShortcutHandlers = {
  onPlayPause: () => void;
  onSeek: (direction: number) => void;
  onMute: () => void;
  onFullscreen: () => void;
  onClose: () => void;
  onVolumeUp?: () => void;
  onVolumeDown?: () => void;
};

const STICK_ACTIVATE = 0.7;
const STICK_RELEASE = 0.35;
const REPEAT_DELAY = 300;

/** D-pad directional state derived from layout-aware button/hat reading. */
interface DpadState {
  seekLeft: boolean;
  seekRight: boolean;
  volUp: boolean;
  volDown: boolean;
}

/**
 * Map a hat-axis value to directional flags. The 8BitDo Pro 3 hat rests at
 * 3.286 (not 0); the eight octants map to the same seek/volume intent as the
 * discrete D-pad buttons.
 */
function hatToDpad(hat: number, neutral: number): DpadState {
  const state: DpadState = { seekLeft: false, seekRight: false, volUp: false, volDown: false };
  if (Math.abs(hat - neutral) <= 0.2) return state;
  if (hat <= -0.85) state.volUp = true;
  else if (hat <= -0.6) { state.volUp = true; state.seekRight = true; }
  else if (hat <= -0.3) state.seekRight = true;
  else if (hat <= -0.1) { state.volDown = true; state.seekRight = true; }
  else if (hat < 0.3) state.volDown = true;
  else if (hat < 0.6) { state.volDown = true; state.seekLeft = true; }
  else if (hat < 0.85) state.seekLeft = true;
  else if (hat <= 1.1) { state.volUp = true; state.seekLeft = true; }
  return state;
}

/** Read D-pad directions from a gamepad (buttons or hat axis). */
function readDpad(gamepad: Gamepad, layout: GamepadLayout): DpadState {
  if (layout.dpadButtons) {
    return {
      seekLeft: Boolean(gamepad.buttons[layout.dpadButtons.left]?.pressed),
      seekRight: Boolean(gamepad.buttons[layout.dpadButtons.right]?.pressed),
      volUp: Boolean(gamepad.buttons[layout.dpadButtons.up]?.pressed),
      volDown: Boolean(gamepad.buttons[layout.dpadButtons.down]?.pressed),
    };
  }
  if (layout.hatAxis !== undefined) {
    const hat = gamepad.axes[layout.hatAxis];
    if (hat === undefined) return { seekLeft: false, seekRight: false, volUp: false, volDown: false };
    return hatToDpad(hat, layout.hatNeutral ?? 0);
  }
  return { seekLeft: false, seekRight: false, volUp: false, volDown: false };
}

/** Hysteresis check: stick must exceed activate, then drop below release. */
function stickActive(activeInputs: Set<string>, gid: number, name: string, value: number): boolean {
  const key = `${gid}:${name}`;
  return activeInputs.has(key) ? Math.abs(value) >= STICK_RELEASE : Math.abs(value) > STICK_ACTIVATE;
}

/** Fire on the rising edge of a button press (one-shot). */
function edgeTrigger(
  activeInputs: Set<string>, gid: number, name: string, pressed: boolean, action: () => void,
): void {
  const key = `${gid}:${name}`;
  if (pressed && !activeInputs.has(key)) {
    activeInputs.add(key);
    action();
  } else if (!pressed) {
    activeInputs.delete(key);
  }
}

/** Fire on press, then repeat at REPEAT_DELAY while held. */
function dirTrigger(
  activeInputs: Set<string>,
  lastFireTime: Record<string, number>,
  gid: number,
  name: string,
  pressed: boolean,
  now: number,
  action: () => void,
): void {
  const key = `${gid}:${name}`;
  if (!pressed) {
    activeInputs.delete(key);
    delete lastFireTime[key];
    return;
  }
  const lastFire = lastFireTime[key] ?? 0;
  if (!activeInputs.has(key)) {
    activeInputs.add(key);
    lastFireTime[key] = now;
    action();
  } else if (now - lastFire >= REPEAT_DELAY) {
    lastFireTime[key] = now;
    action();
  }
}

const KEY_HANDLERS: Record<string, (h: PlayerShortcutHandlers, ev: KeyboardEvent) => void> = {
  ' ': (h, ev) => { ev.preventDefault(); h.onPlayPause(); },
  k: (h, ev) => { ev.preventDefault(); h.onPlayPause(); },
  ArrowLeft: (h, ev) => { ev.preventDefault(); h.onSeek(-1); },
  ArrowRight: (h, ev) => { ev.preventDefault(); h.onSeek(1); },
  ArrowUp: (h, ev) => { ev.preventDefault(); h.onVolumeUp?.(); },
  ArrowDown: (h, ev) => { ev.preventDefault(); h.onVolumeDown?.(); },
  m: (h, ev) => { ev.preventDefault(); h.onMute(); },
  f: (h, ev) => { ev.preventDefault(); h.onFullscreen(); },
  Escape: (h) => h.onClose(),
};

export const playerShortcuts: Action<HTMLElement, PlayerShortcutHandlers> = (node, initialHandlers) => {
  let handlers = initialHandlers;
  const activeInputs = new Set<string>();
  const lastFireTime: Record<string, number> = {};
  let rafId = 0;

  function onKeydown(event: KeyboardEvent) {
    if (event.target instanceof HTMLInputElement || event.target instanceof HTMLTextAreaElement) {
      return;
    }
    const handle = KEY_HANDLERS[event.key];
    if (handle) handle(handlers, event);
  }

  /** Dispatch one gamepad's input through the trigger primitives. */
  function dispatchGamepad(gamepad: Gamepad, now: number) {
    const layout = detectLayout(gamepad);
    const gid = gamepad.index;

    const confirm = Boolean(gamepad.buttons[layout.confirm]?.pressed);
    const cancel = Boolean(gamepad.buttons[layout.cancel]?.pressed);
    const dpad = readDpad(gamepad, layout);

    // Left stick X → seek; right stick Y → volume (hysteresis-gated).
    const sx = gamepad.axes[layout.leftStick[0]] ?? 0;
    const rsy = gamepad.axes[layout.rightStick[1]] ?? 0;
    const seekLeft = dpad.seekLeft || (sx < 0 && stickActive(activeInputs, gid, 'seekLeft', sx));
    const seekRight = dpad.seekRight || (sx > 0 && stickActive(activeInputs, gid, 'seekRight', sx));

    edgeTrigger(activeInputs, gid, 'play', confirm, () => handlers?.onPlayPause());
    edgeTrigger(activeInputs, gid, 'close', cancel, () => handlers?.onClose());
    dirTrigger(activeInputs, lastFireTime, gid, 'seekLeft', seekLeft, now, () => handlers?.onSeek(-1));
    dirTrigger(activeInputs, lastFireTime, gid, 'seekRight', seekRight, now, () => handlers?.onSeek(1));
    dirTrigger(activeInputs, lastFireTime, gid, 'volUp', dpad.volUp, now, () => handlers?.onVolumeUp?.());
    dirTrigger(activeInputs, lastFireTime, gid, 'volDown', dpad.volDown, now, () => handlers?.onVolumeDown?.());
    dirTrigger(activeInputs, lastFireTime, gid, 'rstick_volUp', rsy < 0 && stickActive(activeInputs, gid, 'rstick_volUp', rsy), now, () => handlers?.onVolumeUp?.());
    dirTrigger(activeInputs, lastFireTime, gid, 'rstick_volDown', rsy > 0 && stickActive(activeInputs, gid, 'rstick_volDown', rsy), now, () => handlers?.onVolumeDown?.());
  }

  function pollGamepad() {
    const gamepads = navigator.getGamepads?.() ?? [];
    const now = performance.now();
    for (const gamepad of gamepads) {
      if (gamepad) dispatchGamepad(gamepad, now);
    }
    rafId = requestAnimationFrame(pollGamepad);
  }

  node.addEventListener('keydown', onKeydown);
  rafId = requestAnimationFrame(pollGamepad);

  return {
    update(newHandlers) {
      handlers = newHandlers;
    },
    destroy() {
      node.removeEventListener('keydown', onKeydown);
      cancelAnimationFrame(rafId);
    },
  };
};
