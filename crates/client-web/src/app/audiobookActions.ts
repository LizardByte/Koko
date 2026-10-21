/** Shares listening, hold-to-restart options, and live transport controls across Book and audiobook pages. */
import type { MediaItemSummary, MediaPlaybackTarget } from '../api';
import { activeMusicPlayback } from './musicPlaybackState';
import { toggleCurrentMusic } from './audioPlayer';
import { startPlaybackForItemId, restartAudiobook } from './playbackController';
import { bindResumeHold } from './resumeHold';
import { renderButtonContent } from './ui';
import { state } from './state';
import type { AudiobookRestartScope } from './audiobooks';

/** Only this account's active narration may override its saved listening target. */
export function activeAudiobookId(): number | undefined {
  const playback = activeMusicPlayback();
  return playback && !playback.ended && playback.userId === state.bootstrap?.current_user?.id && playback.item.item_type === 'audiobook_part' ? playback.item.parent_id ?? undefined : undefined;
}

/** The same two restart actions are available even while a narration is actively playing. */
export function renderAudiobookAction(book: MediaItemSummary & { playback_target?: MediaPlaybackTarget | null }, secondary = false): string {
  const playback = activeMusicPlayback(), active = activeAudiobookId() === book.id;
  const continuing = active || book.playback_target?.resume || (book.playback_position_ms ?? 0) > 0 && !book.playback_completed;
  let label = continuing ? 'Resume listening' : 'Listen';
  if (active && !playback!.paused) label = 'Pause listening';
  const icon = active && !playback!.paused ? 'pause' : 'play';
  const subtitle = active ? playback!.item.display_title : undefined;
  return `<div class="listening-action"><button type="button" class="${secondary ? 'secondary-button' : ''}" data-listen-book="${book.id}" data-resume-hold aria-controls="audiobook-progress-menu" aria-expanded="false" title="${label} · Hold for listening options">${renderButtonContent(label, icon, 'start', subtitle)}</button>
    <section id="audiobook-progress-menu" class="resume-playback-menu" aria-label="Listening options" data-nav-section="listening-options" data-nav-label="Listening options" hidden>
      <button type="button" class="secondary-button" data-restart-audiobook="track">${renderButtonContent('Restart track', 'rotate-ccw')}</button>
      <button type="button" class="secondary-button" data-restart-audiobook="book">${renderButtonContent('Restart audiobook', 'rotate-ccw')}</button>
    </section></div>`;
}

/** Injectable actions allow gesture and error handling to be verified independently of audio decoding. */
export interface AudiobookActionHandlers { listen(itemId: number): Promise<void>; restart(itemId: number, scope: AudiobookRestartScope): Promise<void> }
const defaultHandlers: AudiobookActionHandlers = {
  async listen(itemId) {
    if (activeAudiobookId() === itemId && await toggleCurrentMusic()) return;
    await startPlaybackForItemId(itemId);
  },
  restart: restartAudiobook,
};

/** Bind independent reading/listening menus without allowing a held release to start an action. */
export function bindAudiobookActions(signal: AbortSignal, failed: (error: unknown) => void, handlers: AudiobookActionHandlers = defaultHandlers): void {
  const trigger = document.querySelector<HTMLButtonElement>('[data-listen-book]');
  const menu = document.querySelector<HTMLElement>('#audiobook-progress-menu');
  if (!trigger || !menu) return;
  const close = (): void => { menu.hidden = true; trigger.setAttribute('aria-expanded', 'false'); };
  const open = (): void => { menu.hidden = false; trigger.setAttribute('aria-expanded', 'true'); menu.querySelector('button')?.focus({ preventScroll: true }); };
  const perform = (scope?: AudiobookRestartScope): void => {
    close(); trigger.disabled = true;
    const id = Number(trigger.dataset.listenBook);
    const operation = scope ? handlers.restart(id, scope) : handlers.listen(id);
    void operation.catch(failed).finally(() => { trigger.disabled = false; });
  };
  bindResumeHold(trigger, open, signal);
  trigger.addEventListener('resume-options-close', close, { signal });
  trigger.addEventListener('contextmenu', (event) => { event.preventDefault(); open(); }, { signal });
  trigger.addEventListener('click', () => perform(), { signal });
  for (const button of menu.querySelectorAll<HTMLButtonElement>('[data-restart-audiobook]')) button.addEventListener('click', () => perform(button.dataset.restartAudiobook as AudiobookRestartScope), { signal });
  const outside = (event: Event): void => { if (event.target instanceof Node && !menu.contains(event.target) && !trigger.contains(event.target)) close(); };
  document.addEventListener('pointerdown', outside, { signal });
  document.addEventListener('focusin', outside, { signal });
  menu.addEventListener('keydown', (event) => { if (event.key === 'Escape') { event.preventDefault(); close(); trigger.focus({ preventScroll: true }); } }, { signal });
}
