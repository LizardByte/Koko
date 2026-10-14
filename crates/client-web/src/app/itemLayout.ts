/** Reveal the compact identity only when the single playback bar becomes sticky. */
export function bindItemPlaybackBar(
  scroller: HTMLElement,
  anchor: HTMLElement,
  bar: HTMLElement,
  signal: AbortSignal,
): void {
  const identity = bar.querySelector<HTMLElement>('.item-bar-identity');
  const update = (): void => {
    const stuck = anchor.getBoundingClientRect().top < scroller.getBoundingClientRect().top;
    bar.classList.toggle('is-stuck', stuck);
    if (identity) identity.hidden = !stuck;
  };
  scroller.addEventListener('scroll', update, { passive: true, signal });
  globalThis.addEventListener('resize', update, { signal });
  update();
}

/** Size a proportional cover from the collapsed copy, within the responsive width budget. */
export function itemCoverDimensions(copyHeight: number, availableWidth: number, ratio = 2 / 3): { width: number; height: number } {
  const width = Math.max(0, Math.min(copyHeight * ratio, 320, availableWidth * 0.28));
  return { width, height: width / ratio };
}

/** Measure wrapped descriptions and match the cover and fading artwork to the opening copy. */
export function bindItemOpeningLayout(page: HTMLElement, signal: AbortSignal, onRowsSized?: () => void): void {
  const identity = page.querySelector<HTMLElement>('.item-identity');
  const description = page.querySelector<HTMLElement>('.item-description-section');
  const text = description?.querySelector<HTMLElement>('[data-measure-overflow]');
  const toggle = description?.querySelector<HTMLButtonElement>('[data-overflow-toggle]');
  const bar = page.querySelector<HTMLElement>('.item-action-dock');
  const scroller = page.closest<HTMLElement>('.main-shell');
  const rows = Array.from(page.querySelectorAll<HTMLElement>('[data-item-row]'));
  if (!identity || !description || !text || !scroller) return;
  let frame: number | undefined;
  const setSize = (name: string, value: number): void => {
    const next = `${Math.round(value * 10) / 10}px`;
    if (page.style.getPropertyValue(name) !== next) page.style.setProperty(name, next);
  };
  const measure = (): void => {
    frame = undefined;
    if (signal.aborted || !page.isConnected) return;
    const lineHeight = Number.parseFloat(getComputedStyle(text).lineHeight) || 24;
    const collapsedHeight = lineHeight * 6;
    if (toggle) toggle.hidden = text.scrollHeight <= collapsedHeight + 1;
    const title = identity.querySelector<HTMLElement>('.item-title-logo, .item-title-fallback') ?? identity;
    const pageTop = page.getBoundingClientRect().top;
    const titleTop = title.getBoundingClientRect().top;
    const expandedHeight = Math.max(0, text.getBoundingClientRect().height - collapsedHeight);
    const collapsedBottom = description.getBoundingClientRect().bottom - expandedHeight;
    if (getComputedStyle(page).getPropertyValue('--item-match-cover').trim() === '1') {
      const ratio = page.classList.contains('episode-detail-page') ? 16 / 9 : 2 / 3;
      const cover = itemCoverDimensions(collapsedBottom - titleTop, page.clientWidth, ratio);
      setSize('--item-cover-width', cover.width);
      setSize('--item-cover-height', cover.height);
      setSize('--item-cover-top', titleTop - pageTop);
    } else {
      page.style.removeProperty('--item-cover-width');
    }
    const padding = Number.parseFloat(getComputedStyle(scroller).paddingTop) || 0;
    setSize('--item-artwork-height', collapsedBottom - pageTop + padding + 160);
    if (bar) scroller.style.setProperty('--item-bar-height', `${bar.offsetHeight + 12}px`);
    let rowsChanged = false;
    for (const row of rows) {
      const gap = Number.parseFloat(getComputedStyle(row).columnGap) || 16;
      const minimumWidth = row.dataset.itemRow === 'extras' ? 230 : row.clientWidth < 400 ? 120 : 135;
      const count = String(completeRowCardCount(row.clientWidth, minimumWidth, gap));
      if (row.style.getPropertyValue('--item-row-count') !== count) {
        row.style.setProperty('--item-row-count', count);
        rowsChanged = true;
      }
    }
    if (rowsChanged) onRowsSized?.();
  };
  const schedule = (): void => {
    if (frame === undefined) frame = requestAnimationFrame(measure);
  };
  const observer = new ResizeObserver(schedule);
  [identity, description, scroller, ...rows, ...(bar ? [bar] : [])].forEach((element) => observer.observe(element));
  globalThis.addEventListener('resize', schedule, { signal });
  signal.addEventListener('abort', () => {
    observer.disconnect();
    if (frame !== undefined) cancelAnimationFrame(frame);
  }, { once: true });
  schedule();
}

/** Fit a whole number of cards into one row, preserving usable sizes at each viewport width. */
export function completeRowCardCount(width: number, minimumCardWidth: number, gap: number): number {
  return Math.max(1, Math.floor((width + gap) / (minimumCardWidth + gap)));
}
