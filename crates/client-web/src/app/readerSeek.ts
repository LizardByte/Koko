/** Keeps slider previews aligned with rendered destinations and coalesces rapid reading-position changes. */
import { readingFraction } from './readerSupport';

/** Engines predict page destinations once the document's current layout is known. */
export interface ReaderSeekActions { seek(fraction: number): Promise<void>; preview(fraction: number): string | undefined }

/** Scrub the document while dragging, showing the actual settled location instead of an estimated text page. */
export function bindReaderSeek(range: HTMLInputElement, preview: HTMLOutputElement, actions: ReaderSeekActions, signal: AbortSignal): (fraction: number, label: string, position?: number) => void {
  let dragging = false, working = false, pending: number | undefined, requested: number | undefined;
  let rendered = { label: '', position: 0 };
  const listen = (target: EventTarget, name: string, listener: EventListener): void => target.addEventListener(name, listener, { signal });
  const value = (): number => readingFraction(Number(range.value) / Number(range.max));
  const show = (fraction: number, label: string): void => {
    preview.hidden = false; preview.textContent = label;
    preview.style.setProperty('--reader-preview-position', `${Math.max(5, Math.min(95, fraction * 100))}%`);
  };
  const restore = (): void => {
    range.value = String(Math.round(rendered.position * Number(range.max)));
    range.setAttribute('aria-valuetext', rendered.label);
  };
  const advance = async (): Promise<void> => {
    if (pending === undefined || signal.aborted) return;
    const fraction = pending; pending = undefined;
    await actions.seek(fraction);
    if (!signal.aborted && pending === undefined && rendered.label) {
      range.setAttribute('aria-valuetext', rendered.label);
      if (!preview.hidden) show(fraction, rendered.label);
    }
    // Start the first seek immediately, then consume the newest destination after layout settles.
    return advance();
  };
  const drain = async (): Promise<void> => {
    if (working) return;
    working = true;
    try {
      await advance();
    } finally { working = false; if (!signal.aborted && !dragging) restore(); }
  };
  const seek = (): void => {
    const fraction = value();
    if (fraction === requested) return;
    requested = fraction; pending = fraction;
    show(fraction, actions.preview(fraction) ?? 'Opening location…');
    // The reader owns error presentation; a failed operation must also release the scrubber.
    void drain().catch(() => { if (!signal.aborted) { pending = undefined; preview.hidden = true; } });
  };
  listen(range, 'pointerdown', () => { dragging = true; requested = undefined; });
  listen(window, 'pointerup', () => { dragging = false; if (!working) restore(); });
  listen(range, 'pointercancel', () => { dragging = false; preview.hidden = true; if (!working) restore(); });
  listen(range, 'pointermove', (event) => {
    if (dragging || working) return;
    const rect = range.getBoundingClientRect(), pointer = event as PointerEvent;
    // Use the shared native player range's content box and 16px thumb travel.
    const style = getComputedStyle(range);
    const left = (Number.parseFloat(style.paddingLeft) || 0) + (Number.parseFloat(style.borderLeftWidth) || 0) + 8;
    const right = (Number.parseFloat(style.paddingRight) || 0) + (Number.parseFloat(style.borderRightWidth) || 0) + 8;
    const maximum = Number(range.max);
    const fraction = Math.round(readingFraction((pointer.clientX - rect.left - left) / Math.max(1, rect.width - left - right)) * maximum) / maximum;
    const target = actions.preview(fraction);
    show(fraction, target ? `Go to ${target.replace(/^Page/, 'page')}` : 'Page preview unavailable');
  });
  listen(range, 'pointerleave', () => { if (!dragging) preview.hidden = true; });
  listen(range, 'input', seek);
  listen(range, 'change', () => { if (requested === undefined) seek(); });
  listen(range, 'blur', () => { dragging = false; preview.hidden = true; if (!working) restore(); });
  return (fraction, label, position = fraction) => {
    if (signal.aborted) return;
    rendered = { label, position: readingFraction(position) };
    if (!dragging && !working) restore();
  };
}
