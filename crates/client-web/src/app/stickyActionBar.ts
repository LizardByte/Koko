/** Reveal a compact page identity when actions reach their scroll container's top edge. */
export function bindStickyActionBar(scroller: HTMLElement, anchor: HTMLElement, bar: HTMLElement, signal: AbortSignal): void {
  const identity = bar.querySelector<HTMLElement>('[data-sticky-identity]');
  const update = (): void => {
    // On narrow screens the document scrolls instead of the main pane.
    const top = Math.max(0, scroller.getBoundingClientRect().top);
    const stuck = anchor.getBoundingClientRect().top < top;
    bar.classList.toggle('is-stuck', stuck);
    if (identity) identity.hidden = !stuck;
  };
  scroller.addEventListener('scroll', update, { passive: true, signal });
  globalThis.addEventListener('scroll', update, { passive: true, signal });
  globalThis.addEventListener('resize', update, { signal });
  update();
}
