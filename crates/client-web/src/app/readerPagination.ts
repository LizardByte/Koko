/** Debounces complete layout pagination and discards counts from obsolete or closed readers. */

import { readerPageIndex, readerPageLabel } from './readerSupport';

/** Dimensions and preferences that determine the rendered pages of a reflowable book. */
export interface ReaderPaginationLayout { width: number; height: number; size: number; direction: string; spread: boolean }

/** Complete rendered pages per spine section; non-linear navigation sections have zero pages. */
export interface ReaderPageMap {
  sections: number[];
  total: number;
  /** Measured columns for each section, including mixed writing modes in a two-page layout. */
  columns: number[];
}

/** Own background work without publishing partial counts or stale layouts after a resize or close. */
export function createReaderPagination(
  measure: (layout: ReaderPaginationLayout, signal: AbortSignal) => Promise<ReaderPageMap>,
  changed: (pages: ReaderPageMap) => void,
  delay = 250,
): { schedule(layout: ReaderPaginationLayout): void; close(): void } {
  let key = '', closed = false, timer: ReturnType<typeof setTimeout> | undefined, active: AbortController | undefined;
  return {
    schedule(layout) {
      if (closed || layout.width <= 0 || layout.height <= 0) return;
      const next = JSON.stringify(layout);
      if (next === key) return;
      key = next; clearTimeout(timer); active?.abort();
      const controller = new AbortController(); active = controller;
      timer = setTimeout(() => {
        void measure(layout, controller.signal).then((pages) => {
          const valid = Number.isInteger(pages.total) && pages.total > 0 && pages.total <= 100000
            && pages.sections.every((count) => Number.isInteger(count) && count >= 0)
            && pages.columns.length === pages.sections.length
            && pages.columns.every((columns, index) => columns === 1 || columns === 2 || columns === 0 && pages.sections[index] === 0)
            && pages.sections.reduce((sum, count) => sum + count, 0) === pages.total;
          if (!closed && !controller.signal.aborted && valid) changed(pages);
        }).catch(() => { /* An optional count failure must not interrupt reading or erase a cached count. */ });
      }, delay);
    },
    close() { closed = true; clearTimeout(timer); active?.abort(); },
  };
}

/** Label visible columns with the same whole-book total used on the library card. */
export function readerTextPageLabel(pages: ReaderPageMap, section: number, screen: number, columns: number): string | undefined {
  const count = pages.sections[section];
  if (!count) return undefined;
  const before = pages.sections.slice(0, section).reduce((sum, count) => sum + count, 0);
  // A two-column section starts with a generated blank followed by its first (recto) page.
  const blank = Number(columns === 2), first = Math.max(0, screen * columns - blank);
  const last = Math.min(count - 1, screen * columns + columns - 1 - blank);
  const start = Math.min(count - 1, first);
  const visible = start === last ? [before + start] : [before + start, before + last];
  return readerPageLabel(visible, pages.total);
}

/** Use Foliate's section anchor and measured spread geometry to predict the page selected by a seek. */
export function readerTextPagePreview(pages: ReaderPageMap, section: number, anchor: number): string | undefined {
  const count = pages.sections[section], columns = pages.columns[section];
  if (!count || !columns) return undefined;
  const screens = Math.ceil((count + Number(columns === 2)) / columns);
  return readerTextPageLabel(pages, section, readerPageIndex(anchor, screens), columns);
}

/** Count occupied columns, excluding the generated blank recto column and an empty final half-spread. */
export function readerColumnPages(extent: number, pitch: number, blank: boolean): number {
  if (!Number.isFinite(extent) || !Number.isFinite(pitch) || extent < 0 || pitch <= 0) throw new Error('Page layout unavailable.');
  // Round away subpixel noise at an exact column boundary.
  return Math.max(1, Math.ceil((extent - 0.5) / pitch) - Number(blank));
}

/** Use actual navigation screens for single pages and cross-check occupied columns for spreads. */
export function verifiedReaderPages(columns: number, screenCount: number, columnsPerScreen: number, blank: boolean): number {
  if (!Number.isInteger(screenCount) || screenCount < 0) throw new Error('Page layout unavailable.');
  // In single-page mode the paginator already has the exact count used by Next/Previous.
  if (columnsPerScreen === 1) return screenCount;
  if (screenCount === 0) return 0; // Empty spine sections do not add a navigable page.
  if (Math.ceil((columns + Number(blank)) / columnsPerScreen) !== screenCount) throw new Error('Page count does not match rendered navigation.');
  return columns;
}
