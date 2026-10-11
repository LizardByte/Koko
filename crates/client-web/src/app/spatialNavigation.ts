/** Geometry shared by keyboard and controller navigation. */
export type NavigationDirection = 'left' | 'right' | 'up' | 'down';

export interface NavigationRect {
  left: number;
  right: number;
  top: number;
  bottom: number;
}

/** Prefer the next control in the same row or column, then the nearest diagonal. */
export function findSpatialTarget<T>(
  origin: NavigationRect,
  candidates: readonly T[],
  rectFor: (candidate: T) => NavigationRect,
  direction: NavigationDirection,
): T | undefined {
  const horizontal = direction === 'left' || direction === 'right';
  const sign = direction === 'right' || direction === 'down' ? 1 : -1;
  const centerX = (origin.left + origin.right) / 2;
  const centerY = (origin.top + origin.bottom) / 2;
  let best: T | undefined;
  let bestScore = Infinity;
  for (const candidate of candidates) {
    const rect = rectFor(candidate);
    const dx = (rect.left + rect.right) / 2 - centerX;
    const dy = (rect.top + rect.bottom) / 2 - centerY;
    const forward = (horizontal ? dx : dy) * sign;
    const originSize = horizontal ? origin.right - origin.left : origin.bottom - origin.top;
    const targetSize = horizontal ? rect.right - rect.left : rect.bottom - rect.top;
    if (forward <= Math.max(1, Math.min(originSize, targetSize) / 2)) continue;
    const overlap = horizontal
      ? Math.min(origin.bottom, rect.bottom) - Math.max(origin.top, rect.top)
      : Math.min(origin.right, rect.right) - Math.max(origin.left, rect.left);
    if (horizontal && overlap <= 0) continue;
    const sideways = Math.abs(horizontal ? dy : dx);
    const score = forward + sideways * 2 + (overlap > 0 ? 0 : 10000);
    if (score < bestScore) {
      best = candidate;
      bestScore = score;
    }
  }
  return best;
}
