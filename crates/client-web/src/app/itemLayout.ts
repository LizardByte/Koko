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

/** Share the responsive cover width and portrait backdrop reference across item types. */
export function itemCoverDimensions(availableWidth: number): { width: number; height: number } {
  const width = Math.max(0, Math.min(300, availableWidth * 0.24));
  return { width, height: width * 1.5 };
}

/** Size each horizontal shelf to show only complete cards. */
function sizeItemRows(rows: HTMLElement[]): boolean {
  let changed = false;
  for (const row of rows) {
    const gap = Number.parseFloat(getComputedStyle(row).columnGap) || 16;
    let minimumWidth = row.clientWidth < 400 ? 120 : 135;
    if (['extras', 'episodes'].includes(row.dataset.itemRow ?? '')) minimumWidth = 230;
    else if (row.dataset.itemRow === 'seasons') minimumWidth = 180;
    const count = String(completeRowCardCount(row.clientWidth, minimumWidth, gap));
    if (row.style.getPropertyValue('--item-row-count') !== count) {
      row.style.setProperty('--item-row-count', count);
      changed = true;
    }
  }
  return changed;
}

/** Measure wrapped descriptions and keep shared artwork dimensions stable as content changes. */
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
    const cover = itemCoverDimensions(page.clientWidth);
    if (getComputedStyle(page).getPropertyValue('--item-match-cover').trim() === '1') {
      setSize('--item-cover-width', cover.width);
      const coverHeight = page.classList.contains('episode-detail-page') ? cover.width * 9 / 16 : cover.height;
      setSize('--item-cover-height', coverHeight);
      setSize('--item-cover-top', titleTop - pageTop);
    } else {
      page.style.removeProperty('--item-cover-width');
    }
    const padding = Number.parseFloat(getComputedStyle(scroller).paddingTop) || 0;
    setSize('--item-artwork-height', Math.max(520, cover.height + padding * 2 + 160));
    const viewport = scroller.getBoundingClientRect();
    setSize('--item-artwork-left', viewport.left);
    setSize('--item-artwork-top', viewport.top);
    setSize('--item-artwork-width', scroller.clientWidth);
    if (bar) scroller.style.setProperty('--item-bar-height', `${bar.offsetHeight + 12}px`);
    if (sizeItemRows(rows)) onRowsSized?.();
  };
  const schedule = (): void => {
    frame ??= requestAnimationFrame(measure);
  };
  const observer = new ResizeObserver(schedule);
  [identity, description, scroller, ...rows, ...(bar ? [bar] : [])].forEach((element) => observer.observe(element));
  globalThis.addEventListener('resize', schedule, { signal });
  signal.addEventListener('abort', () => {
    observer.disconnect();
    if (frame !== undefined) cancelAnimationFrame(frame);
  }, { once: true });
  // Complete the first layout before paint; subsequent resizes are coalesced.
  measure();
  const activeSeason = page.querySelector<HTMLElement>('.season-tab[aria-current="page"]');
  const seasonTabs = activeSeason?.parentElement;
  if (activeSeason && seasonTabs) {
    const activeBounds = activeSeason.getBoundingClientRect();
    const tabBounds = seasonTabs.getBoundingClientRect();
    if (activeBounds.left < tabBounds.left) seasonTabs.scrollLeft += activeBounds.left - tabBounds.left;
    else if (activeBounds.right > tabBounds.right) seasonTabs.scrollLeft += activeBounds.right - tabBounds.right;
  }
}

/** Fit a whole number of cards into one row, preserving usable sizes at each viewport width. */
export function completeRowCardCount(width: number, minimumCardWidth: number, gap: number): number {
  return Math.max(1, Math.floor((width + gap) / (minimumCardWidth + gap)));
}
