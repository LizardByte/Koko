/** Renders account-aware reading actions and reuses the player's hold gesture for resetting progress. */
import { createIcons, icons } from 'lucide';
import { getReaderDocument, saveReadingProgress, type MediaItemDetail, type MediaItemSummary, type ReadingProgress } from '../api';
import { escapeHtml } from './format';
import { bindResumeHold } from './resumeHold';
import { state } from './state';
import { formatBookFormat, formatChildCount, renderButtonContent } from './ui';
import { openBookReader } from './bookReader';
import { activeAudiobookId, renderAudiobookAction } from './audiobookActions';

function readable(item: MediaItemDetail): MediaItemSummary[] {
  const books = ['book', 'comic_issue'].includes(item.item_type) ? [item] : item.children.filter((child) => ['book', 'comic_issue'].includes(child.item_type));
  return books.filter((book) => !book.missing_since);
}
function inProgress(item: MediaItemSummary): boolean { return Boolean(item.reading_progress && item.reading_progress.started !== false && !item.reading_progress.completed); }
function label(item: MediaItemSummary): string {
  if (inProgress(item)) return 'Resume';
  return item.reading_progress?.completed ? 'Read again' : 'Read';
}
function content(item: MediaItemSummary): string {
  const subtitle = inProgress(item) ? `${Math.round(item.reading_progress!.fraction * 100)}% read` : undefined;
  return renderButtonContent(label(item), 'book-open', 'start', subtitle);
}

function listeningInProgress(item: MediaItemSummary): boolean { return (item.playback_position_ms ?? 0) > 0 && !item.playback_completed; }

/** Prefer the most recently started unfinished narration, keeping every version's position independent. */
export function preferredBookAudiobook(item: MediaItemDetail): MediaItemSummary | undefined {
  const versions = item.children.filter((child) => child.item_type === 'audiobook' && !child.missing_since);
  return versions.find((version) => version.id === activeAudiobookId()) ?? versions.filter(listeningInProgress).sort((a, b) => (b.last_played_at ?? 0) - (a.last_played_at ?? 0))[0] ?? versions[0];
}

/** Shared book pages select an ebook format independently of any audiobook narration. */
export function renderBookActions(item: MediaItemDetail): string {
  if (!['book', 'comic_issue', 'book_work', 'comic_series'].includes(item.item_type)) return '';
  const books = readable(item), selected = books.find(inProgress) ?? books[0];
  const options = books.map((book) => `<option value="${book.id}" ${book.id === selected?.id ? 'selected' : ''}>${escapeHtml(formatBookFormat(book))} · ${escapeHtml(book.display_title)}</option>`).join('');
  const formatLabel = item.item_type === 'comic_series' ? 'Issue' : 'Format';
  const formats = books.length > 1 ? `<label class="reader-format-label">${formatLabel}<select data-read-book-format>${options}</select></label>` : '';
  const read = selected ? `<div class="reader-action"><button type="button" data-resume-hold data-read-book="${selected.id}" aria-controls="reader-progress-menu" aria-expanded="false" title="${label(selected)} · Hold for reading options">${content(selected)}</button>
    <section id="reader-progress-menu" class="resume-playback-menu reader-progress-menu" aria-label="Reading options" data-nav-section="reading-options" data-nav-label="Reading options" hidden><button type="button" class="secondary-button" data-reset-reading>${renderButtonContent('Reset reading progress', 'rotate-ccw')}</button></section></div>${formats}` : '';
  const audio = item.item_type === 'book_work' ? preferredBookAudiobook(item) : undefined;
  return read + (audio ? renderAudiobookAction(audio, Boolean(selected)) : '');
}

function selection(): MediaItemSummary | undefined {
  const item = state.selectedItem;
  if (!item) return undefined;
  const id = document.querySelector<HTMLSelectElement>('[data-read-book-format]')?.value;
  const books = readable(item);
  return books.find((book) => book.id === Number(id)) ?? books.find(inProgress) ?? books[0];
}
function refreshAction(): void {
  const book = selection(); if (!book) return;
  for (const button of document.querySelectorAll<HTMLButtonElement>('[data-read-book]')) {
    button.dataset.readBook = String(book.id); button.innerHTML = content(book);
    button.title = `${label(book)} · Hold for reading options`; createIcons({ icons, root: button });
  }
}

/** Update the existing detail actions after a successful save, without rebuilding the reading surface. */
export function updateBookReadingState(itemId: number, progress: ReadingProgress): void {
  const item = state.selectedItem;
  const target = item?.id === itemId ? item : item?.children.find((child) => child.id === itemId);
  if (target) {
    target.reading_progress = progress.locator || progress.page_count ? { fraction: progress.fraction, completed: progress.completed, started: Boolean(progress.locator), page_count: progress.page_count } : null;
    for (const badge of document.querySelectorAll<HTMLElement>(`.media-card[data-item-id="${itemId}"] .media-card-duration`)) { badge.textContent = formatChildCount(target); badge.hidden = !badge.textContent; }
  }
  refreshAction();
}

/** Reset only the reading position; bookmarks and listening history remain intact. */
export async function resetBookReadingProgress(itemId: number): Promise<void> {
  const account = state.bootstrap?.current_user?.id;
  if (account == null) throw new Error('Sign in to reset reading progress.');
  const document = await getReaderDocument(itemId);
  if (state.bootstrap?.current_user?.id !== account) return;
  const progress: ReadingProgress = { revision: document.revision, locator: '', fraction: 0, completed: false, page_count: document.progress?.page_count, bookmarks: document.progress?.bookmarks ?? [] };
  await saveReadingProgress(itemId, progress);
  if (state.bootstrap?.current_user?.id === account) updateBookReadingState(itemId, progress);
}

/** Bind click, keyboard/pointer hold, format switching, and dismissible reading options. */
export function bindBookActions(signal: AbortSignal, failed: (error: unknown) => void): void {
  const button = document.querySelector<HTMLButtonElement>('[data-read-book]');
  const menu = document.querySelector<HTMLElement>('#reader-progress-menu');
  if (!button || !menu) return;
  const close = (): void => { menu.hidden = true; button.setAttribute('aria-expanded', 'false'); };
  const open = (): void => { menu.hidden = false; button.setAttribute('aria-expanded', 'true'); menu.querySelector('button')?.focus(); };
  bindResumeHold(button, open, signal);
  button.addEventListener('resume-options-close', close, { signal });
  button.addEventListener('click', () => { close(); void openBookReader(Number(button.dataset.readBook)).catch(failed); }, { signal });
  button.addEventListener('contextmenu', (event) => { event.preventDefault(); open(); }, { signal });
  menu.querySelector('button')!.addEventListener('click', () => {
    const id = Number(button.dataset.readBook); close(); button.disabled = true;
    void resetBookReadingProgress(id).catch(failed).finally(() => { button.disabled = false; button.focus(); });
  }, { signal });
  document.querySelector('[data-read-book-format]')?.addEventListener('change', () => { close(); refreshAction(); }, { signal });
  const outside = (event: Event): void => { if (event.target instanceof Node && !menu.contains(event.target) && !button.contains(event.target)) close(); };
  document.addEventListener('pointerdown', outside, { signal });
  document.addEventListener('focusin', outside, { signal });
  menu.addEventListener('keydown', (event) => { if (event.key === 'Escape') { event.preventDefault(); close(); button.focus(); } }, { signal });
}
