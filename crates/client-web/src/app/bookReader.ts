/** Owns the accessible book reader, account-scoped resume/bookmarks, appearance, and engine lifecycle. */
import { getReaderDocument, getReaderFile, saveReadingProgress, type ReaderDocument, type ReadingProgress } from '../api';
import { state } from './state';
import type { ReaderEngine, ReaderTarget, ReaderLocation } from './readerSupport';
import { readingFraction } from './readerSupport';
import { createIcons, icons } from 'lucide';
import { bindReaderControls, readerAppearance, readerMarkup, restoreReaderAppearance } from './readerControls';
import { updateBookReadingState } from './bookActions';
import { createReaderFullscreen } from './readerFullscreen';
import { createReaderNotebook } from './readerAnnotations';
import './bookReader.css';

/** Injectable source keeps browser tests on real files without contacting external metadata services. */
export interface ReaderSource {
  document(itemId: number, signal: AbortSignal): Promise<ReaderDocument>;
  file(book: ReaderDocument, signal: AbortSignal): Promise<File>;
  save(itemId: number, progress: ReadingProgress): Promise<void>;
}
const defaultSource: ReaderSource = { document: getReaderDocument, file: getReaderFile, save: saveReadingProgress };
interface Session { account: number; close(save?: boolean): void }
let active: Session | undefined;

/** Close cached content when login changes, even if an unrelated app refresh triggers the change. */
export function syncReaderAccount(): void {
  if (active && state.bootstrap?.current_user?.id !== active.account) active.close(false);
}

export { renderBookActions } from './bookActions';

/** Open a book or comic, restore this account's location, and preserve the reader across app renders. */
export async function openBookReader(itemId: number, source: ReaderSource = defaultSource): Promise<void> {
  const account = state.bootstrap?.current_user?.id;
  if (account == null) throw new Error('Sign in to read books.');
  active?.close();
  const previousFocus = document.activeElement as HTMLElement | null;
  const controller = new AbortController();
  const dialog = document.createElement('dialog'); dialog.className = 'book-reader'; dialog.setAttribute('aria-labelledby', 'book-reader-title');
  dialog.innerHTML = `<div class="book-reader-surface" data-reader-surface>${readerMarkup()}</div>`; document.body.append(dialog); dialog.showModal();
  const fullscreen = createReaderFullscreen(dialog);
  const get = <T extends HTMLElement = HTMLElement>(selector: string): T => dialog.querySelector<T>(selector)!;
  const host = get('.book-reader-viewport');
  const status = (message: string): void => { const notice = get('[data-reader-status]'); notice.textContent = message; notice.classList.toggle('reader-notice', /not saved|failed|unavailable/i.test(message)); };
  let engine: ReaderEngine | undefined, book: ReaderDocument | undefined, progress: ReadingProgress | undefined;
  let closed = false, ready = false, saveTimer: ReturnType<typeof setTimeout> | undefined;
  let panel: 'contents' | 'bookmarks' | 'notes' | 'search' = 'contents', targets: ReaderTarget[] = [], searchGeneration = 0;
  let saves = Promise.resolve();
  let operations = Promise.resolve();
  let resizeObserver: ResizeObserver | undefined;
  let resizeFrame: number | undefined;
  const sameAccount = (): boolean => state.bootstrap?.current_user?.id === account;
  const save = (): void => {
    if (!ready || !progress || !book || !sameAccount()) return;
    const snapshot = structuredClone(progress), id = book.item_id;
    saves = saves.catch(() => undefined).then(async () => {
      if (!sameAccount()) return;
      await source.save(id, snapshot);
      if (sameAccount()) updateBookReadingState(id, snapshot);
      if (!closed) status('Saved');
    }).catch((error: unknown) => { if (!closed) status(`Position not saved: ${error instanceof Error ? error.message : 'request failed'}`); });
  };
  const queueSave = (): void => { clearTimeout(saveTimer); status('Saving…'); saveTimer = setTimeout(save, 900); };
  const disposeEngine = (): void => {
    const current = engine; engine = undefined;
    // A failed decoder can already have released its renderer. Closing must still dismiss the dialog.
    try { current?.close(); } catch { /* The source error remains visible until the reader is dismissed. */ }
  };
  const close = (persist = true): void => {
    if (closed) return;
    clearTimeout(saveTimer);
    if (persist) save();
    closed = true; searchGeneration++; resizeObserver?.disconnect(); controller.abort(); disposeEngine();
    if (resizeFrame !== undefined) cancelAnimationFrame(resizeFrame);
    fullscreen.close();
    dialog.close(); dialog.remove();
    if (active?.close === close) active = undefined;
    if (previousFocus?.isConnected) previousFocus.focus();
  };
  active = { account, close };
  const listen = (target: EventTarget, type: string, handler: EventListener): void => target.addEventListener(type, handler, { signal: controller.signal });
  const run = (operation: () => Promise<void>): Promise<void> => {
    if (!engine || closed) return Promise.resolve();
    operations = operations.then(async () => { if (!closed) await operation(); })
      .catch((error: unknown) => { if (!closed) status(error instanceof Error ? error.message : 'Reader operation failed.'); });
    return operations;
  };
  const renderTargets = (): void => {
    const list = get('[data-reader-targets]');
    list.replaceChildren();
    if (notebook.render(list, panel)) return;
    if (!targets.length) { list.textContent = panel === 'search' ? 'Enter text to search.' : 'No table of contents is available.'; }
    for (const target of targets) {
      const row = document.createElement('div'); row.className = 'reader-target';
      const button = document.createElement('button'); button.type = 'button'; button.textContent = target.label; button.style.paddingInlineStart = `${12 + (target.depth ?? 0) * 14}px`;
      listen(button, 'click', () => run(() => engine!.goTo(target.locator))); row.append(button);
      list.append(row);
    }
    createIcons({ icons, root: list });
  };
  const openPanel = (next: typeof panel): void => {
    panel = next; get('.book-reader-panel').hidden = false;
    get('[data-reader-panel-title]').textContent = panel === 'notes' ? 'Highlights and notes' : panel[0].toUpperCase() + panel.slice(1);
    get<HTMLFormElement>('[data-reader-search-form]').hidden = panel !== 'search';
    targets = panel === 'contents' ? engine?.toc ?? [] : [];
    updatePanelButtons(); renderTargets();
  };
  const notebook = createReaderNotebook(dialog, {
    progress: () => ready && sameAccount() ? progress : undefined,
    open: openPanel,
    navigate: (locator) => { void run(() => engine!.goTo(locator)); },
    changed: () => { queueSave(); void run(() => engine!.annotations?.(progress?.annotations ?? []) ?? Promise.resolve()); renderTargets(); },
    status,
  }, controller.signal);
  const appearance = async (): Promise<void> => {
    const value = readerAppearance(dialog);
    try { localStorage.setItem(`koko-reader-appearance:${account}`, JSON.stringify(value)); } catch { /* Private browsing may disable preference storage. */ }
    await engine?.appearance(value.size, value.direction, value.spread);
  };
  listen(get('[data-reader-close]'), 'click', () => close());
  listen(dialog, 'cancel', (event) => { event.preventDefault(); close(); });
  listen(get('[data-reader-fullscreen]'), 'click', () => { void fullscreen.toggle().catch(() => status('Fullscreen is unavailable in this browser.')); });
  listen(get('[data-reader-next]'), 'click', () => run(() => engine!.next()));
  listen(get('[data-reader-previous]'), 'click', () => run(() => engine!.previous()));
  listen(get('[data-reader-bookmark]'), 'click', () => notebook.bookmark());
  const locationChanged = bindReaderControls(dialog, {
    appearance: () => run(appearance),
    seek: (fraction) => run(() => engine!.seek(fraction)),
    preview: (fraction) => engine?.preview(fraction),
  }, controller.signal);
  listen(get('[data-reader-search-submit]'), 'click', () => get<HTMLFormElement>('[data-reader-search-form]').requestSubmit());
  listen(get('[data-reader-panel-close]'), 'click', () => { notebook.dismiss(); get('.book-reader-panel').hidden = true; updatePanelButtons(); });
  const updatePanelButtons = (): void => {
    for (const control of dialog.querySelectorAll<HTMLElement>('[data-reader-panel]')) control.setAttribute('aria-expanded', String(!get('.book-reader-panel').hidden && control.dataset.readerPanel === panel));
  };
  for (const button of dialog.querySelectorAll<HTMLElement>('[data-reader-panel]')) listen(button, 'click', () => {
    const next = button.dataset.readerPanel as typeof panel;
    const hide = !get('.book-reader-panel').hidden && panel === next;
    notebook.dismiss(); openPanel(next); get('.book-reader-panel').hidden = hide; updatePanelButtons();
    if (panel === 'search' && !hide) get<HTMLInputElement>('[name="query"]').focus();
  });
  listen(get('[data-reader-search-form]'), 'submit', (event) => {
    event.preventDefault(); const query = get<HTMLInputElement>('[name="query"]').value.trim(); if (!query || !engine) return;
    const token = ++searchGeneration; status('Searching…');
    void engine.search(query).then((found) => { if (closed || token !== searchGeneration) { return; } targets = found; renderTargets(); status(found.length ? `${found.length} results` : 'No matches found.'); })
      .catch((error: unknown) => { if (!closed) status(error instanceof Error ? error.message : 'Search failed.'); });
  });
  listen(window, 'keydown', (event) => {
    const key = event as KeyboardEvent;
    if (key.ctrlKey || key.altKey || key.metaKey || key.target instanceof Element && key.target.closest('input,select,textarea,button')) return;
    const rtl = readerAppearance(dialog).direction === 'rtl';
    let action: (() => Promise<void>) | undefined;
    if (['ArrowRight', 'PageDown', ' '].includes(key.key)) action = () => (key.key === 'ArrowRight' && rtl || key.key === ' ' && key.shiftKey) ? engine!.previous() : engine!.next();
    if (['ArrowLeft', 'PageUp'].includes(key.key)) action = () => key.key === 'ArrowLeft' && rtl ? engine!.next() : engine!.previous();
    if (key.key === 'Home') action = () => engine!.seek(0);
    if (key.key === 'End') action = () => engine!.seek(1);
    if (!action) return;
    key.preventDefault(); key.stopImmediatePropagation(); void run(action);
  });
  listen(window, 'pagehide', () => save());
  const moved = (location: ReaderLocation): void => {
    if (closed || !progress) return;
    progress.page_count = location.pageCount ?? progress.page_count;
    progress.locator = location.locator; progress.fraction = readingFraction(location.fraction); progress.completed = progress.fraction >= 0.995;
    locationChanged(progress.fraction, location.label, location.position);
    if (ready) queueSave();
  };
  try {
    book = await source.document(itemId, controller.signal); if (closed || !sameAccount()) { close(false); return; }
    get('#book-reader-title').textContent = book.title;
    progress = book.progress ? structuredClone(book.progress) : { revision: book.revision, locator: '', fraction: 0, completed: false, bookmarks: [] };
    const file = await source.file(book, controller.signal); if (closed || !sameAccount()) { close(false); return; }
    const loaded = ['pdf', 'cbz', 'archive'].includes(book.format)
      ? await (await import('./readerPaged')).openPagedReader(file, book.format, host, moved, notebook.events)
      : await (await import('./readerText')).openTextReader(file, book.format, host, moved, notebook.events);
    if (closed || !sameAccount()) { loaded.close(); close(false); return; }
    engine = loaded;
    await engine.annotations?.(progress.annotations ?? []);
    progress.page_count = engine.pageCount ?? progress.page_count;
    restoreReaderAppearance(dialog, account);
    const saved = progress.locator, fraction = progress.fraction;
    await appearance();
    await restorePosition(engine, saved, fraction, book.format);
    if (closed) return;
    ready = true; status(''); host.focus();
    targets = engine.toc; renderTargets();
    let width = host.clientWidth, height = host.clientHeight;
    resizeObserver = new ResizeObserver(() => {
      if (width === host.clientWidth && height === host.clientHeight) return;
      width = host.clientWidth; height = host.clientHeight;
      if (resizeFrame !== undefined) cancelAnimationFrame(resizeFrame);
      // Reflow after observer delivery so opening navigation cannot create a resize feedback loop.
      resizeFrame = requestAnimationFrame(() => { resizeFrame = undefined; void run(appearance); });
    });
    resizeObserver.observe(host);
  } catch (error: unknown) {
    if (closed) return;
    disposeEngine(); host.replaceChildren();
    get('[data-reader-location]').textContent = 'Book unavailable';
    const message = document.createElement('p'); message.className = 'book-reader-error'; message.setAttribute('role', 'alert');
    message.textContent = error instanceof Error ? error.message : 'This book could not be opened.'; host.append(message); status('Book unavailable');
  }
}

function initialLocation(format: string): string { return ['pdf', 'cbz', 'archive'].includes(format) ? '0' : ''; }

async function restorePosition(engine: ReaderEngine, saved: string, fraction: number, format: string): Promise<void> {
  try { await engine.goTo(saved || initialLocation(format)); }
  catch (error) { if (!saved) { throw error; } await engine.seek(fraction); }
}
