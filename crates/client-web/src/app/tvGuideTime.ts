/** Defines TV guide windows and resolves broadcasts at an absolute schedule time. */

/** Six hours share one horizontal timeline. */
export const TV_WINDOW_SECONDS = 6 * 3600;
/** Guide columns represent half-hour intervals in UTC seconds. */
export const TV_SLOT_SECONDS = 1800;

/** Keep the current half hour visible, while allowing browsing into future windows. */
export function currentGuideWindow(start = 0, now = Date.now()): number {
  return Math.max(start,Math.floor(now / (TV_SLOT_SECONDS * 1000)) * TV_SLOT_SECONDS);
}

/** Prefer the broadcast containing a time; schedule gaps retain the nearest program. */
export function programAtTime<T extends {starts_at:number;ends_at:number}>(programs: readonly T[], time: number): T | undefined {
  const current = programs.find(p => p.starts_at <= time && p.ends_at > time);
  if (current) return current;
  return [...programs].sort((a,b) => Math.min(Math.abs(a.starts_at-time),Math.abs(a.ends_at-time))
    - Math.min(Math.abs(b.starts_at-time),Math.abs(b.ends_at-time)))[0];
}
