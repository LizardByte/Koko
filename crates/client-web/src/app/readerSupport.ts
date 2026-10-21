/** Defines reader engines and removes executable and network-active content from book documents. */
import DOMPurify from 'dompurify';
import type { UponSanitizeElementHook } from 'dompurify';
import type { ReadingAnnotation } from '../api';

/** A selection captured before focus moves from the book to the annotation editor. */
export interface ReaderSelection {
  locator: string;
  quote: string;
  rects?: NonNullable<ReadingAnnotation['rects']>;
}

/** Selection and saved-highlight activation from an engine's book surface. */
export interface ReaderAnnotationEvents {
  selection(value?: ReaderSelection): void;
  activate(locator: string): void;
}

/** A chapter, page, or bookmark target suitable for the reader navigation panel. */
export interface ReaderTarget { label: string; locator: string; depth?: number }

/** Position reported by an engine; CFI and page indices remain independent of audio offsets. */
export interface ReaderLocation {
  locator: string; fraction: number; label: string;
  /** Start of the visible page for the slider, independent of progress through its end. */
  position?: number;
  /** Whole-book pages, including completed background pagination of the current text layout. */
  pageCount?: number;
}

/** Shared operations for paginated text, PDF pages, and comic archives. */
export interface ReaderEngine {
  toc: ReaderTarget[];
  /** Intrinsic/publisher pages or a fully paginated text layout, never byte-based location totals. */
  readonly pageCount?: number;
  next(): Promise<void>;
  previous(): Promise<void>;
  goTo(locator: string): Promise<void>;
  seek(fraction: number): Promise<void>;
  appearance(size: number, direction: string, spread: boolean): Promise<void>;
  /** Predict the destination page/spread; reflowable text waits for completed layout pagination. */
  preview(fraction: number): string | undefined;
  search(query: string): Promise<ReaderTarget[]>;
  /** Replace this account's visible highlights without changing the book's DOM anchors. */
  annotations?(values: readonly ReadingAnnotation[]): Promise<void>;
  close(): void;
}

/** Publisher page anchors can be nested and can repeat the same target. */
export interface ReaderPageReference { href?: string; subitems?: ReaderPageReference[] }

/** Count only distinct page anchors supplied by the EPUB publisher. */
export function publisherPageCount(pages?: ReaderPageReference[] | null): number | undefined {
  const anchors = new Set<string>();
  const visit = (items: ReaderPageReference[]): void => {
    for (const item of items) { if (item.href) { anchors.add(item.href); } if (item.subitems) { visit(item.subitems); } }
  };
  visit(pages ?? []); return anchors.size || undefined;
}

/** Lazily complete one operation at a time when a reader shares a frame or bounds page decoding. */
export async function* readerSequence<T, R>(items: Iterable<T>, operation: (item: T) => Promise<R>): AsyncGenerator<R> {
  for (const item of items) yield operation(item);
}

/** Natural path ordering retains chapter/page numbers, including unpadded comic filenames. */
export function compareReaderPaths(a: string, b: string): number {
  return a.localeCompare(b, undefined, { numeric: true, sensitivity: 'base' });
}

/** An archive entry is a page only if its extension is an image and it is not hidden metadata. */
export function isComicPage(path: string): boolean {
  return !path.split(/[\\/]/).some((part) => part.startsWith('.') || part === '__MACOSX') && /\.(?:jpe?g|png|gif|bmp|webp|avif|svg)$/i.test(path);
}

/** Strip active markup before any book-created document is loaded into the renderer's frames. */
export function sanitizeBookDocument(html: string): string {
  const metadata: { viewport?: string; styles: string[] } = { styles: [] };
  // Inspect metadata in DOMPurify's inert tree before these head elements are removed.
  const inspect: UponSanitizeElementHook = (node) => {
    if (node.nodeType !== 1) return;
    const element = node as Element;
    if (element.localName === 'meta' && element.getAttribute('name')?.toLowerCase() === 'viewport') {
      metadata.viewport ??= element.getAttribute('content') ?? undefined;
    }
    if (element.localName === 'link' && element.getAttribute('rel')?.toLowerCase().split(/\s+/).includes('stylesheet')) {
      const href = element.getAttribute('href')?.trim();
      if (href && /^(?:blob:|data:text\/css[;,])/i.test(href)) metadata.styles.push(href);
    }
  };
  DOMPurify.addHook('uponSanitizeElement', inspect);
  let clean: Node;
  try {
    clean = DOMPurify.sanitize(html, { WHOLE_DOCUMENT: false, FORCE_BODY: true, RETURN_DOM: true,
      FORBID_TAGS: ['script', 'iframe', 'object', 'embed', 'form', 'base', 'meta', 'link'],
      ALLOWED_URI_REGEXP: /^(?:https?:|blob:|[^:]*$)/i });
  } finally { DOMPurify.removeHook('uponSanitizeElement', inspect); }
  const source = clean.ownerDocument?.documentElement;
  if (!source) throw new Error('Book document unavailable.');
  const doc = document.implementation.createHTMLDocument();
  // Import the sanitized body itself so book-specific classes and styles survive.
  doc.body.replaceWith(doc.importNode(clean, true));
  for (const href of metadata.styles) {
    const link = doc.createElement('link'); link.rel = 'stylesheet'; link.href = href;
    doc.head.append(link);
  }
  const viewport = metadata.viewport;
  if (viewport) {
    const values = new Map(viewport.split(',').map((entry): [string, string] => {
      const [key, value] = entry.trim().toLowerCase().split('='); return [key, value ?? ''];
    }));
    const width = Number(values.get('width')), height = Number(values.get('height'));
    if ([width, height].every((value) => Number.isFinite(value) && value > 0 && value <= 100000)) {
      const meta = doc.createElement('meta'); meta.name = 'viewport'; meta.content = `width=${width},height=${height}`; doc.head.append(meta);
    }
  }
  if (source.lang) doc.documentElement.lang = source.lang;
  doc.documentElement.className = source.className;
  if (['rtl', 'ltr'].includes(source.dir)) doc.documentElement.dir = source.dir;
  preserveSvgViewport(doc);
  for (const link of doc.querySelectorAll('a[href]')) {
    if (/^(?:data|javascript|file):/i.test(link.getAttribute('href') ?? '')) link.removeAttribute('href');
  }
  const policy = doc.createElement('meta');
  policy.httpEquiv = 'Content-Security-Policy';
  policy.content = "default-src 'none'; script-src 'none'; connect-src 'none'; frame-src 'none'; img-src blob: data:; media-src blob: data:; font-src blob: data:; style-src 'unsafe-inline' blob: data:; base-uri 'none'; form-action 'none'";
  doc.head.prepend(policy);
  return `<!doctype html>${doc.documentElement.outerHTML}`;
}

function preserveSvgViewport(doc: Document): void {
  if (doc.querySelector('meta[name="viewport"]')) return;
  const dimensions = doc.querySelector('body > svg')?.getAttribute('viewBox')?.trim().split(/[\s,]+/).map(Number);
  if (dimensions?.length !== 4 || !dimensions.every(Number.isFinite) || dimensions[2] <= 0 || dimensions[3] <= 0) return;
  const viewport = doc.createElement('meta'); viewport.name = 'viewport'; viewport.content = `width=${dimensions[2]},height=${dimensions[3]}`; doc.head.append(viewport);
}

/** Reject stale or nonsensical saved percentages before passing them into a renderer. */
export function readingFraction(value: number): number { return Number.isFinite(value) ? Math.max(0, Math.min(1, value)) : 0; }

/** Page one and the last page sit at the slider's endpoints, using the same mapping for preview and seek. */
export function readerPageIndex(fraction: number, count: number): number { return Math.round(readingFraction(fraction) * Math.max(0, count - 1)); }

/** Invert Foliate's page-start reporting; keep a section's last page inside its own navigation interval. */
export function readerTextPosition(sizes: number[], index: number, fraction: number, pages?: number): number {
  const total = sizes.reduce((sum, size) => sum + size, 0);
  if (!total) return 0;
  let anchor = fraction;
  if (pages && pages > 1) {
    const page = Math.max(0, Math.min(pages - 1, Math.round(fraction * pages)));
    anchor = page / (pages - 1);
    // Fraction 1 selects the next section. The middle of the final rounding interval selects this page.
    if (page === pages - 1) anchor -= 0.25 / (pages - 1);
  }
  return readingFraction((sizes.slice(0, index).reduce((sum, size) => sum + size, 0) + anchor * (sizes[index] ?? 0)) / total);
}

/** Physical book spreads keep the cover on the recto, followed by even/odd page pairs. */
export function readerPageSpread(index: number, count: number, spread: boolean): Array<number | null> {
  const page = Math.max(0, Math.min(count - 1, index));
  if (!spread) return [page];
  if (!page) return [null, 0];
  const left = page % 2 === 0 ? page - 1 : page;
  return [left, left + 1 < count ? left + 1 : null];
}

/** Match progress previews and navigation to the same physical spread boundaries. */
export function readerPageLabel(pages: Array<number | null>, count: number): string {
  const visible = pages.filter((page): page is number => page !== null);
  return visible.length > 1 ? `Pages ${visible[0] + 1}–${visible[1] + 1} of ${count}` : `Page ${visible[0] + 1} of ${count}`;
}
