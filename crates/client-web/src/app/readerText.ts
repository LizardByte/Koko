/** Adapts Foliate pagination for EPUB, Kindle, FictionBook, and sanitized text documents. */
import type { ReaderEngine, ReaderLocation, ReaderTarget } from './readerSupport';
import { readerPageLabel, readerPageSpread, readingFraction, readerTextPosition, sanitizeBookDocument, readerSequence } from './readerSupport';
import { escapeHtml } from './format';
import { createReaderPagination, readerColumnPages, readerTextPageLabel, verifiedReaderPages, type ReaderPageMap, type ReaderPaginationLayout } from './readerPagination';

interface Section { id: string; size?: number; linear?: string; pageSpread?: string; load(): Promise<string>; unload?(): void; createDocument?(): Promise<Document> }
interface Toc { label: string; href: string; subitems?: Toc[] }
interface Book { sections: Section[]; dir?: string; rendition?: { spread?: string }; toc?: Toc[]; pageList?: Toc[]; metadata?: { title?: string }; destroy?(): void; splitTOCHref?(href: string): [string, null]; getTOCFragment?(doc: Document): Element; resolveHref?(href: string): { index: number } }
interface View extends HTMLElement {
  book: Book;
  isFixedLayout: boolean;
  renderer: HTMLElement & { columnCount?: number; pages?: number; scrolled?: boolean; setStyles?(css: string): void };
  lastLocation?: { cfi: string };
  open(book: Book): Promise<void>;
  init(options: { lastLocation?: string; showTextStart?: boolean }): Promise<void>;
  goTo(locator: string | number): Promise<void>;
  goToFraction(fraction: number): Promise<void>;
  next(): Promise<void>; prev(): Promise<void>; close(): void;
  search(options: { query: string }): AsyncIterable<{ label?: string; subitems?: Array<{ cfi: string; excerpt?: { pre?: string; match?: string; post?: string } }> }>;
}

interface Paginator extends HTMLElement {
  columnCount?: number;
  readonly size: number;
  readonly pages: number;
  open(book: Book): void;
  goTo(target: { index: number; anchor: number }): Promise<void>;
  setStyles(css: string): void;
  getContents(): Array<{ index: number; doc: Document }>;
  destroy(): void;
}

function textStyles(size: number, direction: string, recto: boolean): string {
  const blank = recto ? 'body::before{content:""!important;display:block!important;height:1px!important;break-after:column!important;column-span:none!important}' : '';
  return `html{background:#0c111d!important;color:#f4f7fb!important;color-scheme:dark!important}body{font-size:${size}%!important;line-height:1.65!important;direction:${direction}}a{color:inherit}${blank}`;
}

function columnExtent(doc: Document, direction: string, blank: boolean, pitch: number): number {
  const style = doc.defaultView!.getComputedStyle(doc.documentElement);
  const vertical = style.writingMode.startsWith('vertical');
  const range = doc.createRange(); range.selectNodeContents(doc.body);
  const content = range.getBoundingClientRect(), root = doc.documentElement.getBoundingClientRect();
  let start = 0;
  if (!vertical) start = direction === 'rtl' ? root.right - content.right : content.left - root.left;
  return readerColumnPages(start + (vertical ? content.height : content.width), pitch, blank);
}

/** Bound optional measurement work, including browsers that block document frames. */
async function paginationStep<T>(operation: Promise<T>, signal: AbortSignal): Promise<T> {
  signal.throwIfAborted();
  let timer: ReturnType<typeof setTimeout> | undefined;
  let abort = (): void => {};
  try {
    return await Promise.race([operation, new Promise<never>((_, reject) => {
      abort = () => reject(signal.reason);
      signal.addEventListener('abort', abort, { once: true });
      timer = setTimeout(() => reject(new Error('Pagination timed out.')), 15000);
    })]);
  } finally { clearTimeout(timer); signal.removeEventListener('abort', abort); }
}

/** Decode an independent copy so measurement cannot unload resources used by the visible reader. */
export async function measureReaderTextPages(createBook: () => Promise<Book>, host: HTMLElement, layout: ReaderPaginationLayout, signal: AbortSignal): Promise<ReaderPageMap> {
  const book = await createBook(), cleanup = sanitizeBookSections(book);
  const renderer = document.createElement('foliate-paginator') as Paginator;
  let disposed = false;
  renderer.setAttribute('aria-hidden', 'true'); renderer.inert = true;
  renderer.style.cssText = `position:fixed;left:-100000px;top:0;display:block;visibility:hidden;pointer-events:none;width:${layout.width}px;height:${layout.height}px`;
  const dispose = (): void => {
    disposed = true;
    // A frame blocked before its first load has no internal view to destroy.
    try { renderer.destroy(); } catch { /* Still release all decoded resources below. */ }
    renderer.remove(); cleanup(); book.destroy?.();
  };
  try {
    signal.throwIfAborted();
    host.parentElement!.append(renderer);
    book.dir = layout.direction; renderer.open(book);
    renderer.setAttribute('max-column-count', layout.spread ? '2' : '1');
    renderer.setAttribute('max-inline-size', layout.spread ? String(Math.max(280, (layout.width - 80) / 2)) : '720');
    renderer.setStyles(textStyles(layout.size, layout.direction, false));
    let count = 0;
    const sections = book.sections.map(() => 0);
    // One paginator owns the frame, so each section must finish before the next navigation.
    const measurements = readerSequence(book.sections.entries(), async ([index, section]): Promise<readonly [number, number]> => {
      if (section.linear === 'no') return [index, 0];
      signal.throwIfAborted();
      const navigation = renderer.goTo({ index, anchor: 0 });
      // Foliate installs its current view only when the frame finishes loading. If canceled
      // in that interval, retry renderer cleanup once the pending view becomes available.
      void navigation.then(() => { if (disposed) { try { renderer.destroy(); } catch { /* Already disposed. */ } } }, () => {});
      await paginationStep(navigation, signal);
      const content = renderer.getContents().find((entry) => entry.index === index);
      if (!content) throw new Error('Page layout unavailable.');
      const blank = layout.spread && renderer.columnCount === 2;
      renderer.setStyles(textStyles(layout.size, layout.direction, blank));
      await paginationStep(content.doc.fonts.ready, signal);
      // Font loading and Foliate's style/resize handlers finish before reading column geometry.
      await paginationStep(new Promise<void>((resolve) => requestAnimationFrame(() => requestAnimationFrame(() => resolve()))), signal);
      const columns = renderer.columnCount ?? 1;
      const occupied = columns === 1 ? 0 : columnExtent(content.doc, layout.direction, blank, renderer.size / columns);
      return [index, verifiedReaderPages(occupied, renderer.pages - 2, columns, blank)];
    });
    for await (const [index, pages] of measurements) {
      sections[index] = pages;
      count += pages;
      if (count > 100000) throw new Error('Page count limit exceeded.');
    }
    return { sections, total: count };
  } finally { dispose(); }
}

/** Wrap every section before rendering so scripts cannot run with the application's origin. */
export function sanitizeBookSections(book: Book): () => void {
  const urls = new Set<string>();
  let closed = false;
  for (const section of book.sections) {
    const load = section.load.bind(section), unload = section.unload?.bind(section);
    let cached: string | undefined;
    section.load = async () => {
      if (cached) return cached;
      const original = await load();
      const html = await (await fetch(original)).text();
      if (closed) throw new Error('Reader closed.');
      cached = URL.createObjectURL(new Blob([sanitizeBookDocument(html)], { type: 'text/html' }));
      urls.add(cached); return cached;
    };
    section.unload = () => { if (cached) { urls.delete(cached); URL.revokeObjectURL(cached); cached = undefined; } unload?.(); };
  }
  return () => { closed = true; for (const url of urls) { URL.revokeObjectURL(url); } urls.clear(); };
}

/** Some embedded browsers block document frames; never leave an opening operation pending forever. */
async function navigate(operation: Promise<void>): Promise<void> {
  let timer: ReturnType<typeof setTimeout> | undefined;
  try { await Promise.race([operation, new Promise<never>((_, reject) => { timer = setTimeout(() => reject(new Error('The book page could not load. Try opening Koko in a full browser.')), 15000); })]); }
  finally { clearTimeout(timer); }
}

function textBook(file: File, raw: string, format: string): Book {
  const html = format === 'text' ? `<html><body><pre style="white-space:pre-wrap;font:inherit">${escapeHtml(raw)}</pre></body></html>` : raw;
  const url = URL.createObjectURL(new Blob([html], { type: 'text/html' }));
  return { metadata: { title: file.name }, sections: [{ id: 'text', size: file.size, load: () => Promise.resolve(url),
    createDocument: () => Promise.resolve().then(() => new DOMParser().parseFromString(sanitizeBookDocument(html), 'text/html')) }],
    splitTOCHref: (href) => [href, null], getTOCFragment: (doc) => doc.documentElement,
    resolveHref: () => ({ index: 0 }), destroy: () => URL.revokeObjectURL(url) };
}

/** Open a reflowable book while preserving CFIs across font, direction, and spread changes. */
export async function openTextReader(file: File, format: string, host: HTMLElement, changed: (location: ReaderLocation) => void): Promise<ReaderEngine> {
  const { makeBook } = await import('foliate-js/view.js');
  const raw = ['text', 'html'].includes(format) ? await file.text() : undefined;
  const createBook = async (): Promise<Book> => raw === undefined ? await makeBook(file) as Book : textBook(file, raw, format);
  const book = await createBook();
  const cleanup = sanitizeBookSections(book);
  const view = document.createElement('foliate-view') as View;
  let size = 100, direction = 'ltr', spread = false, lastStyles = '', position = 0, pageLayout = '';
  let initialized = false, closed = false, pageCount: number | undefined;
  let lastLocation: ReaderLocation | undefined, pageMap: ReaderPageMap | undefined;
  let sectionIndex = 0, screen = 0, columns = 1;
  const sizes = book.sections.map((section) => section.linear !== 'no' && (section.size ?? 0) > 0 ? section.size! : 0);
  const trackPosition = (): void => view.renderer.addEventListener('relocate', (event) => {
    const location = (event as CustomEvent<{ index: number; fraction: number }>).detail;
    // Foliate's public view fraction includes the visible page's end; navigation uses its start.
    // Capture the renderer event before the view emits its enriched relocation.
    const pages = !view.isFixedLayout && !view.renderer.scrolled ? (view.renderer.pages ?? 0) - 2 : undefined;
    position = readerTextPosition(sizes, location.index, location.fraction, pages);
    sectionIndex = location.index;
    screen = pages ? Math.max(0, Math.round(location.fraction * pages)) : 0;
    columns = view.renderer.columnCount ?? 1;
  }, { capture: true });
  const applyStyles = (): void => {
    // A generated column keeps each section's first page on the recto without adding CFI nodes.
    const css = textStyles(size, direction, spread && view.renderer.columnCount === 2);
    if (css !== lastStyles) { lastStyles = css; view.renderer.setStyles?.(css); }
  };
  const pageLabel = (): string | undefined => {
    if (view.isFixedLayout && pageCount) return readerPageLabel(readerPageSpread(sectionIndex, pageCount, spread), pageCount);
    return pageMap ? readerTextPageLabel(pageMap, sectionIndex, screen, columns) : undefined;
  };
  view.className = 'reader-text-view'; host.replaceChildren(view);
  view.addEventListener('external-link', (event) => {
    event.preventDefault();
    const href = (event as CustomEvent<{ href: string }>).detail.href;
    if (/^https?:\/\//i.test(href)) window.open(href, '_blank', 'noopener,noreferrer');
  });
  view.addEventListener('relocate', (event) => {
    const location = (event as CustomEvent<{ cfi: string; fraction?: number; location?: { current: number; total: number }; tocItem?: Toc }>).detail;
    applyStyles();
    lastLocation = { locator: location.cfi, fraction: readingFraction(location.fraction ?? 0), position, label: pageLabel() ?? location.tocItem?.label ?? 'Calculating pages…', pageCount };
    changed(lastLocation);
  });
  view.addEventListener('load', (event) => {
    const doc = (event as CustomEvent<{ doc: Document }>).detail.doc;
    doc.addEventListener('keydown', (event) => {
      if (!['ArrowRight', 'ArrowLeft', 'PageDown', 'PageUp', 'Home', 'End', ' '].includes(event.key)) return;
      event.preventDefault();
      window.dispatchEvent(new KeyboardEvent('keydown', { key: event.key, shiftKey: event.shiftKey, ctrlKey: event.ctrlKey, metaKey: event.metaKey, altKey: event.altKey }));
    });
  });
  try { await view.open(book); trackPosition(); }
  catch (error) { view.close(); book.destroy?.(); cleanup(); throw error; }
  pageCount = view.isFixedLayout ? book.sections.length : undefined;
  const pagination = createReaderPagination((layout, signal) => measureReaderTextPages(createBook, host, layout, signal), (pages) => {
    pageMap = pages; pageCount = pages.total;
    if (lastLocation) { lastLocation = { ...lastLocation, pageCount, label: pageLabel() ?? lastLocation.label }; changed(lastLocation); }
  });
  const paginate = (): void => {
    if (initialized && !view.isFixedLayout) pagination.schedule({ width: host.clientWidth, height: host.clientHeight, size, direction, spread });
  };
  const invalidatePages = (): void => {
    const nextLayout = JSON.stringify({ width: host.clientWidth, height: host.clientHeight, size, direction, spread });
    if (nextLayout !== pageLayout) { pageMap = undefined; pageLayout = nextLayout; }
  };
  const toc: ReaderTarget[] = [];
  const visit = (items: Toc[], depth = 0): void => { for (const item of items) { toc.push({ label: item.label, locator: item.href, depth }); visit(item.subitems ?? [], depth + 1); } };
  visit(book.toc ?? []);
  return { toc, get pageCount() { return pageCount; },
    next: () => view.next(), previous: () => view.prev(),
    async goTo(locator) { if (!initialized) { initialized = true; await navigate(view.init({ lastLocation: locator || undefined })); paginate(); } else await navigate(view.goTo(locator)); },
    seek: (fraction) => navigate(view.goToFraction(readingFraction(fraction))),
    async appearance(nextSize, nextDirection, nextSpread) {
      const reopen = direction !== nextDirection || view.isFixedLayout && (spread !== nextSpread || book.rendition?.spread !== (nextSpread ? 'auto' : 'none'));
      const saved = view.lastLocation?.cfi;
      size = nextSize; direction = nextDirection; spread = nextSpread; book.dir = direction;
      invalidatePages();
      if (view.isFixedLayout) { book.rendition = { ...book.rendition, spread: spread ? 'auto' : 'none' }; book.sections[0].pageSpread = direction === 'rtl' ? 'left' : 'right'; }
      if (reopen) { view.close(); await view.open(book); trackPosition(); }
      view.renderer.setAttribute('max-column-count', spread ? '2' : '1');
      view.renderer.setAttribute('max-inline-size', spread ? String(Math.max(280, (host.clientWidth - 80) / 2)) : '720');
      if (view.isFixedLayout) view.renderer.setAttribute('zoom', size === 100 ? 'fit-page' : String(size / 100));
      lastStyles = ''; applyStyles();
      if (reopen && initialized) await navigate(view.init({ lastLocation: saved }));
      paginate();
    },
    preview: () => undefined,
    async search(query) {
      const results: ReaderTarget[] = [];
      for await (const group of view.search({ query })) {
        for (const item of group.subitems ?? []) {
          results.push({ label: [item.excerpt?.pre, item.excerpt?.match, item.excerpt?.post].filter(Boolean).join('') || group.label || query, locator: item.cfi });
          if (results.length >= 100) return results;
        }
      }
      return results;
    },
    close() {
      if (closed) return;
      closed = true; pagination.close();
      try { view.close(); } finally { cleanup(); host.replaceChildren(); book.destroy?.(); }
    },
  };
}
