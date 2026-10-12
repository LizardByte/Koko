let observedHeader: HTMLElement | null | undefined;
let headerResizeObserver: ResizeObserver | undefined;

/** Reserve the measured pinned header height when revealing media controls during navigation. */
export function syncPinnedHomeHeader(): void {
  const header = document.querySelector<HTMLElement>('.home-pinned-header');
  if (header === observedHeader) return;
  headerResizeObserver?.disconnect();
  headerResizeObserver = undefined;
  observedHeader = header;
  const style = document.documentElement.style;
  if (!header) {
    style.removeProperty('--home-pinned-header-height');
    return;
  }

  const updateHeight = () => {
    if (header === observedHeader) {
      style.setProperty('--home-pinned-header-height', `${header.getBoundingClientRect().height}px`);
    }
  };
  updateHeight();
  headerResizeObserver = new ResizeObserver(updateHeight);
  headerResizeObserver.observe(header);
}
