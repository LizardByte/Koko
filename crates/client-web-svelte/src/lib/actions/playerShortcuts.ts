// Player keyboard shortcuts action (Opportunity E).
// Usage: <div use:playerShortcuts={{ onPlayPause, onSeek, onMute, onFullscreen, onClose }}>
//
// Binds keyboard shortcuts matching vanilla playbackController.ts:
//   Space / k → play/pause
//   ArrowLeft → seek back (escalating)
//   ArrowRight → seek forward (escalating)
//   m → mute toggle
//   f → fullscreen toggle
//   Escape → close player

import type { Action } from 'svelte/action';

export type PlayerShortcutHandlers = {
  onPlayPause: () => void;
  onSeek: (direction: number) => void;
  onMute: () => void;
  onFullscreen: () => void;
  onClose: () => void;
};

export const playerShortcuts: Action<HTMLElement, PlayerShortcutHandlers> = (node, handlers) => {
  function onKeydown(event: KeyboardEvent) {
    // Don't intercept when the user is typing in an input.
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

  node.addEventListener('keydown', onKeydown);

  return {
    update(newHandlers) {
      handlers = newHandlers;
    },
    destroy() {
      node.removeEventListener('keydown', onKeydown);
    },
  };
};
