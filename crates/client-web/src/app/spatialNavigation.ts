/** Geometry shared by keyboard and controller navigation. */
export type NavigationDirection = 'left' | 'right' | 'up' | 'down';

export interface NavigationRect {
  left: number;
  right: number;
  top: number;
  bottom: number;
}

interface ProjectedRect {
  start: number;
  end: number;
  crossStart: number;
  crossEnd: number;
}

/** Express either orientation along the movement axis and its perpendicular axis. */
function projectRect(rect: NavigationRect, horizontal: boolean): ProjectedRect {
  if (horizontal) return { start: rect.left, end: rect.right, crossStart: rect.top, crossEnd: rect.bottom };
  return { start: rect.top, end: rect.bottom, crossStart: rect.left, crossEnd: rect.right };
}

/** Score a candidate, rejecting backwards movement and horizontal jumps between rows. */
function spatialScore(origin: ProjectedRect, target: ProjectedRect, horizontal: boolean, sign: number): number {
  const forward = ((target.start + target.end) / 2 - (origin.start + origin.end) / 2) * sign;
  const originSize = origin.end - origin.start;
  const targetSize = target.end - target.start;
  if (forward <= Math.max(1, Math.min(originSize, targetSize) / 2)) return Infinity;
  const overlap = Math.min(origin.crossEnd, target.crossEnd) - Math.max(origin.crossStart, target.crossStart);
  if (horizontal && overlap <= 0) return Infinity;
  const sideways = Math.abs((target.crossStart + target.crossEnd) / 2 - (origin.crossStart + origin.crossEnd) / 2);
  return forward + sideways * 2 + (overlap > 0 ? 0 : 10000);
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
  const projectedOrigin = projectRect(origin, horizontal);
  let best: T | undefined;
  let bestScore = Infinity;
  for (const candidate of candidates) {
    const score = spatialScore(projectedOrigin, projectRect(rectFor(candidate), horizontal), horizontal, sign);
    if (score < bestScore) {
      best = candidate;
      bestScore = score;
    }
  }
  return best;
}
