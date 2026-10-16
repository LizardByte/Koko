export const RESUME_HOLD_MS = 650;

/** Open playback options at the hold threshold; consume release so it cannot start playback. */
export function bindResumeHold(button: HTMLButtonElement, openOptions: () => void, signal: AbortSignal): void {
  let timer: ReturnType<typeof setTimeout> | undefined;
  let source: 'pointer' | 'keyboard' | 'gamepad' | undefined;
  let opened = false;
  let suppressClick = false;
  let keyboardRelease: string | undefined;
  let pointerOrigin: { x: number; y: number } | undefined;
  const hint = button.closest('.item-primary-action')?.querySelector<HTMLElement>('#resume-hold-hint');
  const resetFeedback = (): void => {
    button.classList.remove('is-holding', 'is-hold-ready');
    if (hint) hint.textContent = 'Hold for playback options';
  };
  const clearTimer = (): void => {
    if (timer !== undefined) clearTimeout(timer);
    timer = undefined;
  };
  const cancel = (): void => {
    if (source === 'pointer') suppressClick = true;
    clearTimer();
    source = undefined;
    pointerOrigin = undefined;
    opened = false;
    resetFeedback();
  };
  const begin = (nextSource: NonNullable<typeof source>): void => {
    if (source || button.disabled || signal.aborted) return;
    source = nextSource;
    opened = suppressClick = false;
    button.classList.add('is-holding');
    if (hint) hint.textContent = 'Keep holding…';
    timer = setTimeout(() => {
      timer = undefined;
      opened = true;
      suppressClick = source === 'pointer';
      button.classList.add('is-hold-ready');
      if (hint) hint.textContent = 'Playback options open';
      openOptions();
    }, RESUME_HOLD_MS);
  };
  const finish = (nextSource: NonNullable<typeof source>): void => {
    if (source !== nextSource) return;
    clearTimer();
    const wasOpened = opened;
    source = undefined;
    pointerOrigin = undefined;
    resetFeedback();
    if (nextSource !== 'pointer' && !wasOpened) button.click();
  };
  button.addEventListener('click', (event) => {
    if (!suppressClick) return;
    event.preventDefault();
    event.stopImmediatePropagation();
    suppressClick = false;
  }, { capture: true, signal });
  button.addEventListener('pointerdown', (event) => {
    if (event.button !== 0) return;
    begin('pointer');
    pointerOrigin = { x: event.clientX, y: event.clientY };
    button.setPointerCapture?.(event.pointerId);
  }, { signal });
  button.addEventListener('pointermove', (event) => {
    if (pointerOrigin && Math.hypot(event.clientX - pointerOrigin.x, event.clientY - pointerOrigin.y) > 12) cancel();
  }, { signal });
  button.addEventListener('pointerup', () => finish('pointer'), { signal });
  button.addEventListener('pointerleave', () => { if (source === 'pointer') cancel(); }, { signal });
  button.addEventListener('pointercancel', cancel, { signal });
  button.addEventListener('blur', cancel, { signal });
  button.addEventListener('contextmenu', (event) => event.preventDefault(), { signal });
  button.addEventListener('keydown', (event) => {
    if (!['Enter', ' '].includes(event.key) || event.altKey || event.ctrlKey || event.metaKey) return;
    event.preventDefault();
    if (!event.repeat) {
      keyboardRelease = event.key;
      begin('keyboard');
    }
  }, { signal });
  const finishKeyboard = (event: KeyboardEvent): void => {
    if (event.key !== keyboardRelease) return;
    event.preventDefault();
    event.stopImmediatePropagation();
    keyboardRelease = undefined;
    finish('keyboard');
  };
  button.addEventListener('keyup', finishKeyboard, { signal });
  globalThis.addEventListener('keydown', (event) => {
    if (event.repeat && event.key === keyboardRelease) {
      event.preventDefault();
      event.stopImmediatePropagation();
    }
  }, { capture: true, signal });
  globalThis.addEventListener('keyup', finishKeyboard, { capture: true, signal });
  button.addEventListener('resume-hold-start', () => begin('gamepad'), { signal });
  button.addEventListener('resume-hold-end', () => finish('gamepad'), { signal });
  button.addEventListener('resume-hold-cancel', cancel, { signal });
  globalThis.addEventListener('blur', cancel, { signal });
  signal.addEventListener('abort', () => { keyboardRelease = undefined; cancel(); }, { once: true });
}
