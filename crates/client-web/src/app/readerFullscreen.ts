/** Fullscreens the reader surface because the Fullscreen API excludes dialog elements. */

/** A reader owns only its own fullscreen session, including a request still awaiting browser approval. */
export interface ReaderFullscreen { active(): boolean; toggle(): Promise<void>; close(): void }

/** Keep the modal dialog for focus isolation while promoting its inner reading surface to fullscreen. */
export function createReaderFullscreen(dialog: HTMLDialogElement): ReaderFullscreen {
  const surface = dialog.querySelector<HTMLElement>('[data-reader-surface]')!;
  const owner = dialog.ownerDocument;
  const active = (): boolean => owner.fullscreenElement === surface;
  let closed = false;
  return {
    active,
    async toggle() {
      if (closed) return;
      if (active()) { await owner.exitFullscreen(); return; }
      await surface.requestFullscreen();
      if (closed && active()) await owner.exitFullscreen();
    },
    close() { closed = true; if (active()) void owner.exitFullscreen().catch(() => undefined); },
  };
}
