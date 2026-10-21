/** Captures bounded text selections and scales PDF highlights without changing the source document. */
import type { ReadingAnnotation } from '../api';
import type { ReaderSelection } from './readerSupport';

/** Shared highlight colors are fixed values, never styles supplied by a book or a saved note. */
export const readerHighlightColors: Record<ReadingAnnotation['color'], string> = {
  yellow: '#f6d365', green: '#76d68a', blue: '#80b9fa', pink: '#f49dc4',
};

/** Ignore collapsed, oversized, and whitespace-only selections instead of saving inaccurate anchors. */
export function readerSelectedText(doc: Document): { range: Range; quote: string } | undefined {
  const selection = doc.getSelection();
  if (!selection?.rangeCount || selection.isCollapsed) return;
  const range = selection.getRangeAt(0), quote = range.toString().trim();
  if (quote && quote.length <= 4000) return { range, quote };
}

/** PDF marks use one page's normalized line rectangles; selections across pages are deliberately rejected. */
export function readerPdfSelection(host: HTMLElement): ReaderSelection | undefined {
  const selected = readerSelectedText(host.ownerDocument); if (!selected) return;
  const node = selected.range.startContainer;
  const page = (node.nodeType === Node.ELEMENT_NODE ? node as Element : node.parentElement)?.closest<HTMLElement>('[data-reader-pdf-page]');
  if (!page || !host.contains(page) || !page.contains(selected.range.endContainer)) return;
  const bounds = page.getBoundingClientRect(); if (!bounds.width || !bounds.height) return;
  const rects = Array.from(selected.range.getClientRects()).flatMap((rect) => {
    const x = Math.max(0, (rect.left - bounds.left) / bounds.width), y = Math.max(0, (rect.top - bounds.top) / bounds.height);
    const right = Math.min(1, (rect.right - bounds.left) / bounds.width), bottom = Math.min(1, (rect.bottom - bounds.top) / bounds.height);
    return right > x && bottom > y ? [{ x, y, width: right - x, height: bottom - y }] : [];
  });
  if (!rects.length || rects.length > 200) return;
  return { locator: page.dataset.readerPdfPage!, quote: selected.quote, rects };
}

/** Repaint only this account's marks on currently visible PDF pages after layout or notebook changes. */
export function drawReaderPdfHighlights(host: HTMLElement, marks: readonly ReadingAnnotation[], activate?: (locator: string) => void): void {
  for (const page of host.querySelectorAll<HTMLElement>('[data-reader-pdf-page]')) {
    page.querySelector('.reader-pdf-highlights')?.remove();
    const overlay = document.createElement('div'); overlay.className = 'reader-pdf-highlights';
    for (const mark of marks.filter((value) => value.locator === page.dataset.readerPdfPage)) {
      for (const rect of mark.rects ?? []) {
        const line = document.createElement('button'); line.type = 'button'; line.className = 'reader-pdf-highlight';
        line.style.cssText = `left:${rect.x * 100}%;top:${rect.y * 100}%;width:${rect.width * 100}%;height:${rect.height * 100}%`;
        line.style.backgroundColor = readerHighlightColors[mark.color];
        line.title = mark.note || mark.quote; line.setAttribute('aria-label', `Edit highlight: ${(mark.note || mark.quote).slice(0, 100)}`);
        line.addEventListener('click', () => activate?.(mark.id)); overlay.append(line);
      }
    }
    page.append(overlay);
  }
}
