/** Binds settings layout controls and conditional fields without discarding unsaved edits. */
import { state } from './state';
import { bindStickyActionBar } from './stickyActionBar';

/** Keep the settings actions visible without covering fields revealed by keyboard focus. */
export function bindSettingsActions(signal: AbortSignal): void {
  const podcastsEnabled = document.querySelector<HTMLInputElement>('[data-podcasts-enabled]');
  const podcastFields = document.querySelector<HTMLElement>('[data-podcast-enabled-settings]');
  podcastsEnabled?.addEventListener('change', () => { if (podcastFields) podcastFields.hidden = !podcastsEnabled.checked; }, { signal });
  const bar = document.querySelector<HTMLElement>('.settings-action-dock');
  const anchor = document.querySelector<HTMLElement>('.settings-actions-anchor');
  const scroller = bar?.closest<HTMLElement>('.main-shell');
  if (!bar || !anchor || !scroller) return;
  bindStickyActionBar(scroller, anchor, bar, signal);
  const updateHeight = (): void => {
    scroller.style.setProperty('--settings-action-height', `${bar.offsetHeight + 16}px`);
  };
  const observer = new ResizeObserver(updateHeight);
  observer.observe(bar);
  signal.addEventListener('abort', () => {
    observer.disconnect();
    scroller.style.removeProperty('--settings-action-height');
  }, { once: true });
  updateHeight();
  bar.querySelectorAll<HTMLButtonElement>('[data-settings-create]').forEach((button) => bindCreationPanel(button, bar, signal));
}

function bindCreationPanel(button: HTMLButtonElement, bar: HTMLElement, signal: AbortSignal): void {
  const kind = button.dataset.settingsCreate;
  if (kind !== 'libraries' && kind !== 'users') return;
  const panelId = button.getAttribute('aria-controls');
  const panel = panelId ? document.getElementById(panelId) : null;
  if (!panel) return;
  const defaultActions = bar.querySelector<HTMLElement>('[data-settings-default-actions]');
  const creationActions = bar.querySelector<HTMLElement>('[data-settings-creation-actions]');
  const setOpen = (open: boolean): void => {
    state.settingsCreation = open ? kind : undefined;
    panel.hidden = !open;
    button.setAttribute('aria-expanded', String(open));
    if (defaultActions) defaultActions.hidden = open;
    if (creationActions) creationActions.hidden = !open;
    if (open) {
      panel.scrollIntoView({ block: 'start', behavior: 'instant' });
      panel.querySelector<HTMLInputElement>('input')?.focus({ preventScroll: true });
    } else {
      button.focus({ preventScroll: true });
    }
  };
  button.addEventListener('click', () => setOpen(Boolean(panel.hidden)), { signal });
  bar.querySelector<HTMLButtonElement>('[data-close-settings-create]')?.addEventListener('click', () => setOpen(false), { signal });
  panel.addEventListener('keydown', (event) => {
    if (event.key !== 'Escape' || event.defaultPrevented) return;
    event.preventDefault();
    setOpen(false);
  }, { signal });
}
