// Player keyboard + gamepad shortcuts action (Opportunity E).
// Usage: <div use:playerShortcuts={{ onPlayPause, onSeek, onMute, onFullscreen, onClose }}>
//
// Keyboard: Space/k (play/pause), arrows (seek), m (mute), f (fullscreen), Escape (close)
// Gamepad: A=play/pause, B=close, left/right=seek (escalating), up/down=volume
//
// Gamepad polling runs via rAF only while the player action is mounted
// (the global spatialNavigation action skips gamepad when player is open).

import type { Action } from 'svelte/action';

export type PlayerShortcutHandlers = {
  onPlayPause: () => void;
  onSeek: (direction: number) => void;
  onMute: () => void;
  onFullscreen: () => void;
  onClose: () => void;
  onVolumeUp?: () => void;
  onVolumeDown?: () => void;
};

const STICK_ACTIVATE = 0.4;
const STICK_RELEASE = 0.25;

export const playerShortcuts: Action<HTMLElement, PlayerShortcutHandlers> = (node, initialHandlers) => {
  let handlers = initialHandlers;
  const activeButtons = new Set<string>();
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
    for (const gamepad of gamepads) {
      if (!gamepad) continue;

      const aButton = Boolean(gamepad.buttons[0]?.pressed);
      const bButton = Boolean(gamepad.buttons[1]?.pressed);
      const dpadLeft = Boolean(gamepad.buttons[14]?.pressed);
      const dpadRight = Boolean(gamepad.buttons[15]?.pressed);
      const dpadUp = Boolean(gamepad.buttons[12]?.pressed);
      const dpadDown = Boolean(gamepad.buttons[13]?.pressed);

      const stickX = gamepad.axes[0] ?? 0;
      const stickY = gamepad.axes[1] ?? 0;
      const stickActive = (name: string, value: number) =>
        activeButtons.has(`${gamepad!.index}:${name}`)
          ? Math.abs(value) < STICK_RELEASE
          : Math.abs(value) > STICK_ACTIVATE;

      const actions: Array<[string, boolean, () => void]> = [
        ['play', aButton, () => handlers?.onPlayPause()],
        ['close', bButton, () => handlers?.onClose()],
        ['seekLeft', dpadLeft || (stickX < 0 && stickActive('seekLeft', stickX)), () => handlers?.onSeek(-1)],
        ['seekRight', dpadRight || (stickX > 0 && stickActive('seekRight', stickX)), () => handlers?.onSeek(1)],
        ['volUp', dpadUp || (stickY < 0 && stickActive('volUp', stickY)), () => handlers?.onVolumeUp?.()],
        ['volDown', dpadDown || (stickY > 0 && stickActive('volDown', stickY)), () => handlers?.onVolumeDown?.()],
      ];

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
