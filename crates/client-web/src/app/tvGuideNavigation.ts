/** Keeps keyboard and gamepad guide navigation at a stable broadcast time across channel rows. */
import type { NavigationDirection } from './spatialNavigation';
import { programAtTime, TV_WINDOW_SECONDS } from './tvGuideTime';

interface GuidePosition {
  source: string;
  key: string;
  time: number;
  programColumn: boolean;
  control: HTMLElement;
}
let position: GuidePosition | undefined;

function key(control: HTMLElement): string {
  return `${control.dataset.tvSelectChannel}:${control.dataset.tvSelectProgram ?? ''}`;
}

function clampTime(guide: HTMLElement, time: number): number {
  const start = Number(guide.dataset.tvWindowStart);
  return Math.max(start,Math.min(time,start+TV_WINDOW_SECONDS-1));
}

function remember(guide: HTMLElement, control: HTMLElement, time: number, programColumn = control.classList.contains('tv-guide-program')): void {
  position = {source:guide.dataset.tvGuideSource ?? '',key:key(control),time:clampTime(guide,time),control,programColumn};
}

/** Initialize an anchor for direct focus without resetting one established by directional input. */
export function rememberTvGuideFocus(control: HTMLElement | null): void {
  const guide = control?.closest<HTMLElement>('#tv-guide-scroll');
  if (!guide || !control?.hasAttribute('data-tv-select-channel')) { position = undefined; return; }
  if (position && position.source === guide.dataset.tvGuideSource && position.key === key(control)) {
    remember(guide,control,position.time,position.programColumn);
  } else remember(guide,control,Number(control.dataset.tvStart ?? guide.dataset.tvWindowStart));
}

/** A pointer can select a time within a long broadcast, rather than always choosing its beginning. */
export function rememberTvGuidePointer(event: PointerEvent): void {
  const control = (event.target as Element | null)?.closest<HTMLElement>('.tv-guide-program');
  const guide = control?.closest<HTMLElement>('#tv-guide-scroll');
  const track = control?.closest('.tv-guide-track')?.getBoundingClientRect();
  if (control && guide && track?.width) remember(guide,control,Number(guide.dataset.tvWindowStart)+(event.clientX-track.left)/track.width*TV_WINDOW_SECONDS);
}

function rowPrograms(row: Element): HTMLElement[] {
  return Array.from(row.querySelectorAll<HTMLElement>('.tv-guide-program'));
}

function programForRow(row: Element, time: number): HTMLElement | undefined {
  const programs = rowPrograms(row).map(control => ({control,starts_at:Number(control.dataset.tvStart),ends_at:Number(control.dataset.tvEnd)}));
  return programAtTime(programs,time)?.control ?? row.querySelector<HTMLElement>('.tv-channel-select') ?? undefined;
}

function horizontalTarget(guide: HTMLElement, row: Element, control: HTMLElement, direction: 'left' | 'right', time: number): {target?:HTMLElement} | undefined {
  if (!control.classList.contains('tv-guide-program')) return;
  const programs = rowPrograms(row), index = programs.indexOf(control);
  let target: HTMLElement | undefined = programs[index+(direction === 'right' ? 1 : -1)];
  if (!target && direction === 'left') target = row.querySelector<HTMLElement>('.tv-channel-select') ?? undefined;
  if (target?.dataset.tvStart) time = direction === 'right' ? Number(target.dataset.tvStart) : Number(target.dataset.tvEnd)-1;
  if (target) remember(guide,target,time);
  return {target};
}

/** Resolve a row or adjacent broadcast using the remembered time, never a card's center. */
export function tvGuideNavigationTarget(control: HTMLElement, direction: NavigationDirection): {target?:HTMLElement} | undefined {
  const guide = control.closest<HTMLElement>('#tv-guide-scroll'), row = control.closest('.tv-guide-row');
  if (!guide || !row || !control.hasAttribute('data-tv-select-channel')) return;
  rememberTvGuideFocus(control);
  const time = position!.time;
  if (direction === 'left' || direction === 'right') return horizontalTarget(guide,row,control,direction,time);
  const rows = Array.from(guide.querySelectorAll('.tv-guide-row'));
  const nextRow = rows[rows.indexOf(row)+(direction === 'down' ? 1 : -1)];
  if (!nextRow) return;
  const programColumn = position!.programColumn;
  const target = programColumn ? programForRow(nextRow,time) : nextRow.querySelector<HTMLElement>('.tv-channel-select') ?? undefined;
  if (target) remember(guide,target,time,programColumn);
  return {target};
}

/** Return the time whose horizontal visibility should be preserved for a focused cell. */
export function tvGuideNavigationTime(control: HTMLElement): number | undefined {
  const guide = control.closest<HTMLElement>('#tv-guide-scroll');
  return guide && position?.key === key(control) ? clampTime(guide,position.time) : undefined;
}

/** Replace an expired focused program with a program on the same channel after a guide update. */
export function restoreTvGuideFocus(): HTMLElement | undefined {
  // Moving a still-mounted keyed cell can blur it too, even when isConnected
  // remains true. Restore only when no newer control has received focus.
  if (!position || (document.activeElement && document.activeElement !== document.body)) return;
  const guide = document.querySelector<HTMLElement>('#tv-guide-scroll');
  if (!guide || guide.dataset.tvGuideSource !== position.source) { position = undefined; return; }
  const row = Array.from(guide.querySelectorAll('.tv-guide-row')).find(candidate =>
    candidate.querySelector<HTMLElement>('.tv-channel-select')?.dataset.tvSelectChannel === position!.control.dataset.tvSelectChannel);
  const time = clampTime(guide,position.time);
  const target = row ? programForRow(row,time) : undefined;
  if (target) remember(guide,target,time,position.programColumn);
  return target;
}
