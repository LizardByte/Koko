// Player keyboard + gamepad shortcuts action.
// Usage: <div use:playerShortcuts={{ onPlayPause, onSeek, onMute, onFullscreen, onClose }}>
//
// Keyboard: Space/k (play/pause), arrows (seek/volume), m (mute), f (fullscreen), Escape (close)
// Gamepad: A=play/pause, B=close, left/right=seek, up/down=volume (layout-aware)

import type { Action } from 'svelte/action';
import { detectLayout } from '$lib/gamepadLayouts';

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

export const playerShortcuts: Action<HTMLElement, PlayerShortcutHandlers> = (node, initialHandlers) => {
  let handlers = initialHandlers;
  const activeInputs = new Set<string>();
  const lastFireTime: Record<string, number> = {};
  let rafId = 0;

  function onKeydown(event: KeyboardEvent) {
    if (event.target instanceof HTMLInputElement || event.target instanceof HTMLTextAreaElement) {
      return;
    }

    switch (event.key) {
      case ' ':
      case 'k':
        event.preventDefault();
        handlers?.onPlayPause();
        break;
      case 'ArrowLeft':
        event.preventDefault();
        handlers?.onSeek(-1);
        break;
      case 'ArrowRight':
        event.preventDefault();
        handlers?.onSeek(1);
        break;
      case 'ArrowUp':
        event.preventDefault();
        handlers?.onVolumeUp?.();
        break;
      case 'ArrowDown':
        event.preventDefault();
        handlers?.onVolumeDown?.();
        break;
      case 'm':
        event.preventDefault();
        handlers?.onMute();
        break;
      case 'f':
        event.preventDefault();
        handlers?.onFullscreen();
        break;
      case 'Escape':
        event.preventDefault();
        handlers?.onClose();
        break;
    }
  }

  function pollGamepad() {
    const gamepads = navigator.getGamepads?.() ?? [];
    const now = performance.now();
    for (const gamepad of gamepads) {
      if (!gamepad) continue;
      const layout = detectLayout(gamepad);
      const gid = gamepad.index;

      const confirm = Boolean(gamepad.buttons[layout.confirm]?.pressed);
      const cancel = Boolean(gamepad.buttons[layout.cancel]?.pressed);

      // D-pad / left stick for seek + volume
      let seekLeft = false, seekRight = false, volUp = false, volDown = false;

      if (layout.dpadButtons) {
        seekLeft = Boolean(gamepad.buttons[layout.dpadButtons.left]?.pressed);
        seekRight = Boolean(gamepad.buttons[layout.dpadButtons.right]?.pressed);
        volUp = Boolean(gamepad.buttons[layout.dpadButtons.up]?.pressed);
        volDown = Boolean(gamepad.buttons[layout.dpadButtons.down]?.pressed);
      } else if (layout.hatAxis !== undefined) {
        const hatNeutral = layout.hatNeutral ?? 0;
        const hat = gamepad.axes[layout.hatAxis];
        if (hat !== undefined && Math.abs(hat - hatNeutral) > 0.2) {
          if (hat <= -0.85) volUp = true;
          if (hat <= -0.6 && hat > -0.85) { volUp = true; seekRight = true; }
          if (hat <= -0.3 && hat > -0.6) seekRight = true;
          if (hat <= -0.1 && hat > -0.3) { volDown = true; seekRight = true; }
          if (hat >= 0.1 && hat < 0.3) volDown = true;
          if (hat >= 0.3 && hat < 0.6) { volDown = true; seekLeft = true; }
          if (hat >= 0.6 && hat < 0.85) seekLeft = true;
          if (hat >= 0.85 && hat <= 1.1) { volUp = true; seekLeft = true; }
        }
      }

      // Left stick
      const sx = gamepad.axes[layout.leftStick[0]] ?? 0;
      const sy = gamepad.axes[layout.leftStick[1]] ?? 0;
      const stickActive = (name: string, value: number) => {
        const key = `${gid}:${name}`;
        return activeInputs.has(key)
          ? Math.abs(value) >= STICK_RELEASE
          : Math.abs(value) > STICK_ACTIVATE;
      };

      const edgeTrigger = (name: string, pressed: boolean, action: () => void) => {
        const key = `${gid}:${name}`;
        if (pressed && !activeInputs.has(key)) {
          activeInputs.add(key);
          action();
        } else if (!pressed) {
          activeInputs.delete(key);
        }
      };

      const dirTrigger = (name: string, pressed: boolean, action: () => void) => {
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
      };

      edgeTrigger('play', confirm, () => handlers?.onPlayPause());
      edgeTrigger('close', cancel, () => handlers?.onClose());
      dirTrigger('seekLeft', seekLeft || (sx < 0 && stickActive('seekLeft', sx)), () => handlers?.onSeek(-1));
      dirTrigger('seekRight', seekRight || (sx > 0 && stickActive('seekRight', sx)), () => handlers?.onSeek(1));
      dirTrigger('volUp', volUp || (sy < 0 && stickActive('volUp', sy)), () => handlers?.onVolumeUp?.());
      dirTrigger('volDown', volDown || (sy > 0 && stickActive('volDown', sy)), () => handlers?.onVolumeDown?.());
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
