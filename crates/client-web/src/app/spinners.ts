let spinnerVisibilityObserver: IntersectionObserver | undefined;

function visibleSpinnerObserver(): IntersectionObserver | undefined {
  if (!('IntersectionObserver' in globalThis)) {
    return undefined;
  }
  spinnerVisibilityObserver ??= new IntersectionObserver((entries) => {
    entries.forEach((entry) => {
      entry.target.classList.toggle('is-spinner-visible', entry.isIntersecting);
    });
  }, { rootMargin: '120px' });
  return spinnerVisibilityObserver;
}

/** Syncs lazy spinner visibility classes with the viewport. */
export function syncVisibleSpinners(): void {
  const spinners = Array.from(document.querySelectorAll<HTMLElement>('.loading-spinner:not(.player-loading-spinner)'));
  const observer = visibleSpinnerObserver();
  if (!observer) {
    spinners.forEach((spinner) => spinner.classList.add('is-spinner-visible'));
    return;
  }

  observer.disconnect();
  spinners.forEach((spinner) => observer.observe(spinner));
}
