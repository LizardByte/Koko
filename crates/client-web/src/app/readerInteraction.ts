/** Carries book-frame pointer interactions to the reader's surrounding controls. */

/** Book content was pressed inside a document whose native events cannot leave its frame. */
export const readerContentPointerDown = 'reader-content-pointerdown';

/** Notify the outer reader without canceling text selection, links, or annotation interactions. */
export function forwardReaderPointerDown(doc: Document, host: HTMLElement, signal: AbortSignal): void {
  doc.addEventListener('pointerdown', () => {
    host.dispatchEvent(new Event(readerContentPointerDown, { bubbles: true }));
  }, { signal });
}
