/** Presents one player-style reading controller with accessible icon controls and a seek destination preview. */
import { createIcons, icons } from 'lucide';
import type { AppIconName } from './types';
import { renderIcon } from './ui';
import { bindReaderSeek, type ReaderSeekActions } from './readerSeek';

/** Browser-local reading preferences; the reader follows Koko's dark appearance. */
export interface ReaderAppearance { size: number; direction: string; spread: boolean }

function button(attribute: string, label: string, icon: AppIconName, extra = ''): string {
  return `<button type="button" class="player-icon-button" ${attribute} aria-label="${label}" title="${label}" ${extra}>${renderIcon(icon, 'player-control-icon')}</button>`;
}

/** Keep the reading surface free of a second toolbar; all controls belong to the bottom player. */
export function readerMarkup(): string {
  return `<div class="book-reader-main"><aside id="reader-navigation" class="book-reader-panel" hidden aria-label="Book navigation"><div class="reader-panel-heading"><h3 data-reader-panel-title>Contents</h3>${button('data-reader-panel-close', 'Close navigation', 'x')}</div><form data-reader-search-form hidden><label>Search this book<input name="query" type="search" maxlength="200" /></label>${button('data-reader-search-submit', 'Find in book', 'search')}</form><div data-reader-targets></div></aside><div class="book-reader-viewport" tabindex="0" aria-label="Book pages"><p class="book-reader-loading" role="status">Opening book…</p></div></div>
    <footer class="book-reader-footer" aria-label="Reader controls">
      <div class="reader-seek"><output id="reader-seek-preview" class="reader-seek-preview" hidden></output><input class="player-progress" data-reader-position aria-label="Reading position" aria-describedby="reader-seek-preview" type="range" min="0" max="1000" step="1" value="0" /></div>
      <div class="reader-control-row"><div class="reader-identity">${button('data-reader-close', 'Close reader', 'arrow-left')}<div><h2 id="book-reader-title">Opening book…</h2><span data-reader-location>Loading…</span></div></div>
        <div class="reader-transport">${button('data-reader-previous', 'Previous page', 'chevron-left')}${button('data-reader-next', 'Next page', 'chevron-right')}</div>
        <div class="reader-tools">${button('data-reader-options', 'Reading options', 'sliders-horizontal', 'aria-expanded="false" aria-controls="reader-options"')}${button('data-reader-fullscreen', 'Fullscreen', 'maximize')}</div></div>
      <span data-reader-status class="sr-only" role="status" aria-live="polite"></span>
      <section id="reader-options" class="reader-options" aria-label="Reading options" hidden>
        <div class="reader-option-tools">${button('data-reader-panel="contents"', 'Contents', 'list-tree', 'aria-controls="reader-navigation" aria-expanded="false"')}${button('data-reader-panel="bookmarks"', 'Bookmarks', 'bookmark', 'aria-controls="reader-navigation" aria-expanded="false"')}${button('data-reader-bookmark', 'Add bookmark', 'bookmark-plus')}${button('data-reader-panel="search"', 'Search this book', 'search', 'aria-controls="reader-navigation" aria-expanded="false"')}</div>
        <div class="reader-size">${button('data-reader-size-step="-5"', 'Decrease text size or zoom', 'minus')}<label title="Text size or zoom">${renderIcon('a-large-small', 'player-control-icon')}<input data-reader-size class="player-progress" aria-label="Text size or zoom" type="range" min="75" max="220" step="5" value="100" /></label><output data-reader-size-value>100%</output>${button('data-reader-size-step="5"', 'Increase text size or zoom', 'plus')}</div>
        <div class="reader-layout">${button('data-reader-direction="ltr"', 'Right-to-left reading', 'arrow-right', 'aria-pressed="false"')}${button('data-reader-spread', 'Two pages', 'columns-2', 'aria-pressed="false"')}</div>
      </section></footer>`;
}

/** Read toggle values without tying their state to checkbox or select widgets. */
export function readerAppearance(dialog: HTMLElement): ReaderAppearance {
  return { size: Number(dialog.querySelector<HTMLInputElement>('[data-reader-size]')!.value), direction: dialog.querySelector<HTMLElement>('[data-reader-direction]')!.dataset.readerDirection ?? 'ltr', spread: dialog.querySelector('[data-reader-spread]')!.getAttribute('aria-pressed') === 'true' };
}

function showAppearance(dialog: HTMLElement): void {
  const value = readerAppearance(dialog), direction = dialog.querySelector<HTMLButtonElement>('[data-reader-direction]')!;
  dialog.querySelector('[data-reader-size-value]')!.textContent = `${value.size}%`;
  dialog.querySelector<HTMLElement>('[data-reader-size]')!.style.setProperty('--reader-progress', `${(value.size - 75) / 145 * 100}%`);
  direction.setAttribute('aria-pressed', String(value.direction === 'rtl'));
  direction.innerHTML = renderIcon(value.direction === 'rtl' ? 'arrow-left' : 'arrow-right', 'player-control-icon');
  direction.title = value.direction === 'rtl' ? 'Left-to-right reading' : 'Right-to-left reading'; direction.setAttribute('aria-label', direction.title);
  dialog.querySelector('[data-reader-next]')!.innerHTML = renderIcon(value.direction === 'rtl' ? 'chevron-left' : 'chevron-right', 'player-control-icon');
  dialog.querySelector('[data-reader-previous]')!.innerHTML = renderIcon(value.direction === 'rtl' ? 'chevron-right' : 'chevron-left', 'player-control-icon');
  createIcons({ icons, root: dialog });
}

/** Restore safe preferences while ignoring the retired standalone theme setting. */
export function restoreReaderAppearance(dialog: HTMLElement, account: number): void {
  try {
    const stored = JSON.parse(localStorage.getItem(`koko-reader-appearance:${account}`) ?? '{}') as Partial<ReaderAppearance>;
    if (typeof stored.size === 'number' && stored.size >= 75 && stored.size <= 220) dialog.querySelector<HTMLInputElement>('[data-reader-size]')!.value = String(stored.size);
    if (stored.direction === 'ltr' || stored.direction === 'rtl') dialog.querySelector<HTMLElement>('[data-reader-direction]')!.dataset.readerDirection = stored.direction;
    dialog.querySelector('[data-reader-spread]')!.setAttribute('aria-pressed', String(stored.spread === true));
  } catch { /* Malformed preferences do not prevent reading. */ }
  showAppearance(dialog);
}

/** Reader-owned operations keep this controller independent of the document format. */
export interface ReaderChromeActions extends ReaderSeekActions { appearance(): void }

/** Bind icon toggles and seek previews, returning a function to update the saved reading location. */
export function bindReaderChrome(dialog: HTMLElement, actions: ReaderChromeActions, signal: AbortSignal): (fraction: number, label: string, position?: number) => void {
  const get = <T extends HTMLElement = HTMLElement>(selector: string): T => dialog.querySelector<T>(selector)!;
  const options = get('#reader-options'), trigger = get<HTMLButtonElement>('[data-reader-options]');
  const range = get<HTMLInputElement>('[data-reader-position]'), preview = get<HTMLOutputElement>('#reader-seek-preview');
  const updatePosition = bindReaderSeek(range, preview, actions, signal);
  const listen = (target: EventTarget, name: string, listener: EventListener): void => target.addEventListener(name, listener, { signal });
  const hideOptions = (): void => { options.hidden = true; trigger.setAttribute('aria-expanded', 'false'); };
  const changed = (): void => { showAppearance(dialog); actions.appearance(); };
  listen(trigger, 'click', () => { options.hidden = !options.hidden; trigger.setAttribute('aria-expanded', String(!options.hidden)); });
  listen(dialog, 'pointerdown', (event) => { if (event.target instanceof Node && !options.contains(event.target) && !trigger.contains(event.target)) hideOptions(); });
  listen(dialog, 'keydown', (event) => { const key = event as KeyboardEvent; if (key.key === 'Escape' && !options.hidden) { key.preventDefault(); key.stopPropagation(); hideOptions(); trigger.focus(); } });
  for (const control of dialog.querySelectorAll('[data-reader-panel],[data-reader-bookmark]')) listen(control, 'click', hideOptions);
  listen(get('[data-reader-size]'), 'change', changed);
  for (const control of dialog.querySelectorAll<HTMLButtonElement>('[data-reader-size-step]')) listen(control, 'click', () => {
    const size = get<HTMLInputElement>('[data-reader-size]'); size.value = String(Math.max(75, Math.min(220, Number(size.value) + Number(control.dataset.readerSizeStep)))); changed();
  });
  listen(get('[data-reader-direction]'), 'click', () => { const control = get('[data-reader-direction]'); control.dataset.readerDirection = control.dataset.readerDirection === 'rtl' ? 'ltr' : 'rtl'; changed(); });
  listen(get('[data-reader-spread]'), 'click', () => { const control = get('[data-reader-spread]'); control.setAttribute('aria-pressed', String(control.getAttribute('aria-pressed') !== 'true')); changed(); });
  listen(document, 'fullscreenchange', () => {
    const control = get('[data-reader-fullscreen]'), fullscreen = document.fullscreenElement === get('[data-reader-surface]');
    control.innerHTML = renderIcon(fullscreen ? 'minimize' : 'maximize', 'player-control-icon'); control.title = fullscreen ? 'Exit fullscreen' : 'Fullscreen'; control.setAttribute('aria-label', control.title); createIcons({ icons, root: control });
  });
  createIcons({ icons, root: dialog });
  return (fraction, label, position) => {
    get('[data-reader-location]').textContent = `${label} · ${Math.round(fraction * 100)}%`;
    updatePosition(fraction, label, position);
  };
}
