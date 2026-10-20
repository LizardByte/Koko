/** Renders fixed PDF and comic pages with lazy extraction, zoom, spreads, and persistent page indices. */
import type { ReaderEngine, ReaderLocation, ReaderTarget } from './readerSupport';
import { compareReaderPaths, isComicPage, readerPageIndex, readerPageSpread, readerPageLabel, readerSequence } from './readerSupport';
import pdfWorkerUrl from 'pdfjs-dist/build/pdf.worker.min.mjs?url';
import 'pdfjs-dist/web/pdf_viewer.css';

interface PageSource {
  count: number;
  toc: ReaderTarget[];
  render(index: number, target: HTMLElement, width: number, scale: number): Promise<void>;
  search(query: string): Promise<ReaderTarget[]>;
  close(): void;
}

function assets(path: string): string { return new URL(`${import.meta.env.BASE_URL}assets/reader/${path}`, globalThis.location.href).href; }

/** Visit pages lazily so a bounded PDF search does not allocate the entire page index. */
function* pageNumbers(count: number): Generator<number> {
  for (let page = 1; page <= count; page++) yield page;
}

async function pdfSource(file: File): Promise<PageSource> {
  const pdfjs = await import('pdfjs-dist');
  pdfjs.GlobalWorkerOptions.workerSrc = pdfWorkerUrl;
  const task = pdfjs.getDocument({ data: new Uint8Array(await file.arrayBuffer()),
    cMapUrl: assets('pdfjs/cmaps/'), cMapPacked: true, standardFontDataUrl: assets('pdfjs/standard_fonts/'), wasmUrl: assets('pdfjs/wasm/'), enableXfa: false });
  const pdf = await task.promise.catch(async (error: unknown) => { await task.destroy(); throw error; });
  const running = new Set<ReturnType<Awaited<ReturnType<typeof pdf.getPage>>['render']>>();
  const toc: ReaderTarget[] = [];
  const outline = await pdf.getOutline();
  const visit = async (items: NonNullable<typeof outline>, depth = 0): Promise<void> => {
    for await (const item of items) {
      try {
        const destination = typeof item.dest === 'string' ? await pdf.getDestination(item.dest) : item.dest;
        if (Array.isArray(destination) && destination[0] != null) {
          const index = typeof destination[0] === 'number' ? destination[0] : await pdf.getPageIndex(destination[0]);
          toc.push({ label: item.title, locator: String(index), depth });
        }
      } catch { /* Invalid optional outline entries do not prevent reading a valid PDF. */ }
      await visit(item.items, depth + 1);
    }
  };
  if (outline) await visit(outline);
  return {
    count: pdf.numPages, toc,
    async render(index, target, width, scale) {
      const page = await pdf.getPage(index + 1);
      const natural = page.getViewport({ scale: 1 });
      const height = Math.max(80, (target.closest('.book-reader-viewport')?.clientHeight ?? 800) - 40);
      const ratio = Math.min(width / natural.width, height / natural.height, 2.5) * scale;
      const viewport = page.getViewport({ scale: ratio });
      const pixelRatio = Math.min(devicePixelRatio || 1, 2, Math.sqrt(16000000 / (viewport.width * viewport.height)));
      const canvas = document.createElement('canvas');
      canvas.width = Math.ceil(viewport.width * pixelRatio);
      canvas.height = Math.ceil(viewport.height * pixelRatio);
      canvas.style.width = `${viewport.width}px`; canvas.style.height = `${viewport.height}px`;
      target.style.width = `${viewport.width}px`; target.style.height = `${viewport.height}px`;
      target.style.setProperty('--scale-factor', String(ratio));
      target.append(canvas);
      const render = page.render({ canvas, viewport, transform: [pixelRatio, 0, 0, pixelRatio, 0, 0] });
      running.add(render);
      try { await render.promise; } finally { running.delete(render); }
      if (!target.isConnected) return;
      const text = document.createElement('div'); text.className = 'textLayer'; target.append(text);
      await new pdfjs.TextLayer({ textContentSource: page.streamTextContent(), container: text, viewport }).render();
    },
    async search(query) {
      const results: ReaderTarget[] = [];
      const pages = readerSequence(pageNumbers(pdf.numPages), async (i) => {
        const content = await (await pdf.getPage(i)).getTextContent();
        const text = content.items.map((item) => 'str' in item ? item.str : '').join(' ');
        return { i, text };
      });
      for await (const { i, text } of pages) {
        const match = text.toLocaleLowerCase().indexOf(query.toLocaleLowerCase());
        if (match >= 0) results.push({ label: `Page ${i}: ${text.slice(Math.max(0, match - 35), match + 120)}`, locator: String(i - 1) });
        if (results.length >= 100) break;
      }
      return results;
    },
    close() { for (const render of running) { render.cancel(); } void task.destroy(); },
  };
}

interface ArchiveFile { name: string; size: number; extract(): Promise<File> }
interface ArchiveEntry { path: string; file: ArchiveFile }

async function comicSource(file: File): Promise<PageSource> {
  const { Archive } = await import('libarchive.js');
  Archive.init({ workerUrl: assets('libarchive/worker-bundle.js') });
  const archive = await Archive.open(file);
  try {
    if (await archive.hasEncryptedData()) throw new Error('This comic archive is password protected. Use an unencrypted copy.');
    const entries = (await archive.getFilesArray() as ArchiveEntry[])
      .map((entry) => ({ ...entry, name: `${entry.path}${entry.file.name}` }))
      .filter((entry) => isComicPage(entry.name)).sort((a, b) => compareReaderPaths(a.name, b.name));
    if (!entries.length || entries.length > 20000) throw new Error('The comic has no readable pages or exceeds the page limit.');
    const urls = new Set<string>();
    return {
      count: entries.length,
      toc: entries.map((entry, index) => ({ label: `Page ${index + 1} · ${entry.name}`, locator: String(index) })),
      async render(index, target, width, scale) {
        const entry = entries[index];
        if (entry.file.size > 128 * 1024 * 1024) throw new Error('This comic page exceeds the 128 MiB image limit.');
        const page = await entry.file.extract();
        const type = /\.svg$/i.test(entry.name) ? 'image/svg+xml' : page.type;
        const url = URL.createObjectURL(new Blob([page], { type })); urls.add(url);
        const image = new Image(); image.alt = `Page ${index + 1}`;
        const loaded = new Promise<void>((resolve, reject) => { image.onload = () => resolve(); image.onerror = () => reject(new Error('This comic page could not be decoded.')); });
        image.src = url;
        try {
          await loaded;
          const maxHeight = Math.max(250, (target.closest('.book-reader-viewport')?.clientHeight ?? 800) - 40);
          const fit = Math.min(width / image.naturalWidth, maxHeight / image.naturalHeight, 1);
          image.style.width = `${image.naturalWidth * fit * scale}px`;
          image.style.height = `${image.naturalHeight * fit * scale}px`;
          target.append(image);
        } finally { URL.revokeObjectURL(url); urls.delete(url); }
      },
      search: (query) => Promise.resolve(entries.flatMap((entry, index) => entry.name.toLocaleLowerCase().includes(query.toLocaleLowerCase()) ? [{ label: entry.name, locator: String(index) }] : []).slice(0, 100)),
      close() { for (const url of urls) { URL.revokeObjectURL(url); } void archive.close(); },
    };
  } catch (error) { await archive.close(); throw error; }
}

/** Open a PDF or comic using a page-based engine; the caller owns cleanup and error display. */
export async function openPagedReader(file: File, format: string, host: HTMLElement, changed: (location: ReaderLocation) => void): Promise<ReaderEngine> {
  const source = format === 'pdf' ? await pdfSource(file) : await comicSource(file);
  let index = 0, size = 1, spread = false, direction = 'ltr', generation = 0, closed = false;
  const draw = async (): Promise<void> => {
    const token = ++generation;
    const pair = document.createElement('div'); pair.className = 'reader-page-pair'; pair.dir = direction;
    host.replaceChildren(pair);
    const pages = readerPageSpread(index, source.count, spread);
    const width = Math.max(80, (host.clientWidth - 56) / pages.length);
    pair.style.gridTemplateColumns = `repeat(${pages.length}, ${width}px)`;
    // Create slots in reading order, then render the independent spread pages together.
    await Promise.all(pages.map(async (pageIndex) => {
      if (closed || token !== generation) return;
      const page = document.createElement('div'); page.className = 'reader-fixed-page'; pair.append(page);
      if (pageIndex === null) { page.className = 'reader-blank-page'; page.setAttribute('aria-hidden', 'true'); return; }
      await source.render(pageIndex, page, width, size);
    }));
    if (closed || token !== generation) return;
    host.scrollTop = 0;
    const visible = pages.filter((page): page is number => page !== null);
    changed({ locator: String(index), fraction: (visible[visible.length - 1] + 1) / source.count, position: source.count > 1 ? index / (source.count - 1) : 0, label: readerPageLabel(pages, source.count) });
  };
  const goTo = async (locator: string): Promise<void> => {
    const wanted = Number(locator);
    if (!Number.isInteger(wanted) || wanted < 0 || wanted >= source.count) throw new Error('This page is unavailable.');
    index = readerPageSpread(wanted, source.count, spread).find((page) => page !== null)!; await draw();
  };
  let start: Touch | undefined;
  const begin = (event: TouchEvent): void => { start = event.touches.length === 1 ? event.touches[0] : undefined; };
  const end = (event: TouchEvent): void => {
    const touch = event.changedTouches[0];
    if (start && touch && size <= 1 && Math.abs(touch.clientX - start.clientX) > 70 && Math.abs(touch.clientY - start.clientY) < 50) {
      const forward = (touch.clientX < start.clientX) !== (direction === 'rtl');
      void goTo(String(Math.max(0, Math.min(source.count - 1, index + (forward ? 1 : -1) * (spread ? 2 : 1))))).catch(() => undefined);
    }
    start = undefined;
  };
  host.addEventListener('touchstart', begin, { passive: true }); host.addEventListener('touchend', end, { passive: true });
  return { toc: source.toc, pageCount: source.count,
    next: () => goTo(String(Math.min(source.count - 1, index + (spread && index > 0 ? 2 : 1)))),
    previous: () => goTo(String(Math.max(0, index - (spread ? 2 : 1)))), goTo,
    seek: (fraction) => goTo(String(readerPageIndex(fraction, source.count))),
    async appearance(zoom, readingDirection, twoPages) { size = zoom / 100; direction = readingDirection; spread = twoPages; await goTo(String(index)); },
    preview: (fraction) => readerPageLabel(readerPageSpread(readerPageIndex(fraction, source.count), source.count, spread), source.count),
    search: (query) => source.search(query),
    close() { if (closed) { return; } closed = true; generation++; host.removeEventListener('touchstart', begin); host.removeEventListener('touchend', end); source.close(); host.replaceChildren(); },
  };
}
