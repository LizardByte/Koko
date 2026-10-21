/** Edits this reader's private named bookmarks, anchored highlights, and notes without HTML interpolation. */
import { createIcons, icons } from 'lucide';
import type { ReadingAnnotation, ReadingProgress } from '../api';
import type { ReaderAnnotationEvents, ReaderSelection } from './readerSupport';
import { renderIcon } from './ui';

/** Navigation panels containing personal reading marks. */
export type ReaderNotebookPanel = 'bookmarks' | 'notes';

interface NotebookContext {
  progress(): ReadingProgress | undefined;
  open(panel: ReaderNotebookPanel): void;
  navigate(locator: string): void;
  changed(): void;
  status(message: string): void;
}

/** Bind one session's notebook; the caller's abort signal removes its listeners on account changes. */
export function createReaderNotebook(dialog: HTMLElement, context: NotebookContext, signal: AbortSignal): {
  events: ReaderAnnotationEvents;
  render(list: HTMLElement, panel: string): boolean;
  bookmark(): void;
  dismiss(): void;
} {
  const get = <T extends HTMLElement = HTMLElement>(selector: string): T => dialog.querySelector<T>(selector)!;
  const editor = get<HTMLFormElement>('[data-reader-note-editor]');
  const name = get<HTMLInputElement>('[data-reader-mark-name]');
  const note = get<HTMLTextAreaElement>('[data-reader-mark-note]');
  let selection: ReaderSelection | undefined;
  let draft: { kind: 'bookmark'; locator: string } | { kind: 'annotation'; value: ReadingAnnotation } | undefined;
  const listen = (target: EventTarget, type: string, handler: EventListener): void => target.addEventListener(type, handler, { signal });
  const refreshIcons = (root: HTMLElement): void => { createIcons({ icons, root }); };
  const dismiss = (): void => { draft = undefined; editor.hidden = true; get('[data-reader-targets]').hidden = false; };
  const show = (panel: ReaderNotebookPanel): void => {
    context.open(panel);
    editor.hidden = false; get('[data-reader-targets]').hidden = true;
    get('[data-reader-mark-name-label]').hidden = panel !== 'bookmarks';
    get('[data-reader-mark-note-label]').hidden = panel === 'bookmarks';
    get('[data-reader-mark-colors]').hidden = panel === 'bookmarks';
    get('[data-reader-mark-quote]').textContent = draft?.kind === 'annotation' ? draft.value.quote : '';
    get('[data-reader-mark-quote]').hidden = !get('[data-reader-mark-quote]').textContent;
    const color = draft?.kind === 'annotation' ? draft.value.color : 'yellow';
    for (const button of dialog.querySelectorAll<HTMLElement>('[data-reader-mark-color]')) button.setAttribute('aria-pressed', String(button.dataset.readerMarkColor === color));
    (panel === 'bookmarks' ? name : note).focus();
  };
  const editAnnotation = (value: ReadingAnnotation): void => {
    dismiss(); draft = { kind: 'annotation', value: structuredClone(value) }; note.value = value.note; show('notes');
  };
  const currentAnnotation = (): ReadingAnnotation | undefined => {
    const progress = context.progress();
    if (!progress?.locator) return;
    if ((progress.annotations?.length ?? 0) >= 500) { context.status('Remove a highlight or note before adding more.'); return; }
    return { id: crypto.randomUUID(), locator: selection?.locator ?? progress.locator, quote: selection?.quote ?? '', rects: selection?.rects, note: '', color: 'yellow' };
  };
  const store = (value: ReadingAnnotation): void => {
    const progress = context.progress(); if (!progress) return;
    const marks = progress.annotations ??= [];
    const existing = marks.findIndex((mark) => mark.id === value.id);
    if (existing < 0) marks.push(value); else marks[existing] = value;
    context.changed();
  };
  const action = (label: string, icon: 'pencil' | 'trash-2', handler: () => void): HTMLButtonElement => {
    const button = document.createElement('button'); button.type = 'button'; button.className = 'player-icon-button';
    button.setAttribute('aria-label', label); button.title = label; button.innerHTML = renderIcon(icon, 'player-control-icon');
    listen(button, 'click', handler); return button;
  };
  const renderMark = (mark: ReadingProgress['bookmarks'][number] | ReadingAnnotation, progress: ReadingProgress, panel: ReaderNotebookPanel): HTMLElement => {
    const annotation = 'id' in mark ? mark : undefined;
    const label = 'title' in mark ? mark.title : mark.note || mark.quote;
    const row = document.createElement('div'); row.className = 'reader-target reader-personal-mark';
    if (annotation) row.dataset.markColor = annotation.color;
    const target = document.createElement('button'); target.type = 'button'; target.textContent = label;
    listen(target, 'click', () => context.navigate(mark.locator)); row.append(target);
    row.append(action(`Edit ${panel === 'bookmarks' ? 'bookmark' : 'highlight or note'}`, 'pencil', () => {
      if (annotation) editAnnotation(annotation);
      else { dismiss(); draft = { kind: 'bookmark', locator: mark.locator }; name.value = label; show('bookmarks'); }
    }));
    row.append(action(`Remove ${panel === 'bookmarks' ? 'bookmark' : 'highlight or note'}`, 'trash-2', () => {
      if (annotation) progress.annotations = progress.annotations?.filter((value) => value.id !== annotation.id);
      else progress.bookmarks = progress.bookmarks.filter((value) => value.locator !== mark.locator);
      context.changed(); context.open(panel);
    }));
    return row;
  };
  listen(editor, 'submit', (event) => {
    event.preventDefault(); const progress = context.progress(); if (!progress || !draft) return;
    if (draft.kind === 'bookmark') {
      const title = name.value.trim(); if (!title) { name.focus(); return; }
      const locator = draft.locator;
      const existing = progress.bookmarks.find((mark) => mark.locator === locator);
      if (existing) existing.title = title;
      else if (progress.bookmarks.length < 200) progress.bookmarks.push({ title, locator: draft.locator });
      context.changed();
    } else {
      draft.value.note = note.value.trim();
      if (!draft.value.quote && !draft.value.note) { note.focus(); return; }
      store(draft.value);
    }
    const panel = draft.kind === 'bookmark' ? 'bookmarks' : 'notes'; dismiss(); context.open(panel);
  });
  listen(get('[data-reader-mark-cancel]'), 'click', () => {
    const panel = draft?.kind === 'bookmark' ? 'bookmarks' : 'notes'; dismiss(); context.open(panel);
  });
  for (const button of dialog.querySelectorAll<HTMLElement>('[data-reader-mark-color]')) listen(button, 'click', () => {
    if (draft?.kind !== 'annotation') return;
    draft.value.color = button.dataset.readerMarkColor as ReadingAnnotation['color'];
    for (const control of dialog.querySelectorAll<HTMLElement>('[data-reader-mark-color]')) control.setAttribute('aria-pressed', String(control === button));
  });
  listen(get('[data-reader-highlight]'), 'click', () => {
    if (!selection) return;
    const existing = context.progress()?.annotations?.find((mark) => mark.locator === selection!.locator);
    if (existing) { editAnnotation(existing); return; }
    const value = currentAnnotation(); if (value) { store(value); context.status('Highlight saved.'); }
  });
  for (const button of dialog.querySelectorAll<HTMLElement>('[data-reader-add-note]')) listen(button, 'click', () => {
    const existing = selection && context.progress()?.annotations?.find((mark) => mark.locator === selection!.locator);
    const value = existing || currentAnnotation(); if (value) editAnnotation(value);
  });
  return {
    events: {
      selection(value) {
        selection = value;
        get('[data-reader-selection]').hidden = !value;
        get('[data-reader-selection-quote]').textContent = value?.quote ?? '';
      },
      activate(locator) {
        const mark = context.progress()?.annotations?.find((value) => value.id === locator || value.locator === locator);
        if (mark) editAnnotation(mark);
      },
    },
    bookmark() {
      const progress = context.progress(); if (!progress?.locator) return;
      const existing = progress.bookmarks.find((mark) => mark.locator === progress.locator);
      if (!existing && progress.bookmarks.length >= 200) { context.status('Remove a bookmark before adding more.'); return; }
      dismiss(); draft = { kind: 'bookmark', locator: progress.locator };
      name.value = existing?.title ?? get('[data-reader-location]').textContent?.slice(0, 100) ?? 'Bookmark'; show('bookmarks'); name.select();
    },
    dismiss,
    render(list, panel) {
      if (panel !== 'bookmarks' && panel !== 'notes') return false;
      const progress = context.progress();
      const marks = panel === 'bookmarks' ? progress?.bookmarks ?? [] : progress?.annotations ?? [];
      if (!marks.length) list.textContent = panel === 'bookmarks' ? 'No bookmarks yet.' : 'Select text to highlight it or add a note to this page.';
      if (progress) for (const mark of marks) list.append(renderMark(mark, progress, panel));
      refreshIcons(list); return true;
    },
  };
}
