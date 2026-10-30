/** Positions the shared player over the hero artwork without moving or restarting its video. */
import { state } from './state';

/** Track the hero slot while the same player is minimized, cleaning up on the next binding. */
export function positionTvPreview(): () => void {
  if (!state.activeTvPlayback || state.isPlayerOpen) return () => undefined;
  const slot = document.querySelector<HTMLElement>('#tv-hero-media');
  const overlay = document.querySelector<HTMLElement>('.tv-preview-player');
  if (!slot || !overlay) return () => undefined;
  const update = () => {
    const rect = slot.getBoundingClientRect();
    overlay.style.left = `${rect.left}px`; overlay.style.top = `${rect.top}px`;
    overlay.style.width = `${rect.width}px`; overlay.style.height = `${rect.height}px`;
    overlay.style.visibility = rect.bottom <= 0 || rect.top >= innerHeight ? 'hidden' : 'visible';
  };
  const observer = new ResizeObserver(update); observer.observe(slot);
  document.addEventListener('scroll', update, true); window.addEventListener('resize', update);
  update();
  return () => { observer.disconnect(); document.removeEventListener('scroll', update, true); window.removeEventListener('resize', update); };
}
