interface RouteRendererOptions {
  initial: 'immediate' | 'deferred' | 'none';
  isCurrent: () => boolean;
  isReady: () => boolean;
  render: (preserveDom: boolean) => void;
}

/** Avoid a one-frame loading placeholder when an in-app route is ready quickly. */
export function createRouteRenderer({ initial, isCurrent, isReady, render }: RouteRendererOptions): {
  publish: () => void;
  finish: () => void;
} {
  let painted = initial === 'none';
  let frame: number | undefined;
  let loadingTimer: ReturnType<typeof setTimeout> | undefined;
  const paint = (): void => {
    if (!isCurrent()) return;
    render(painted);
    painted = true;
  };
  const clearLoadingTimer = (): void => {
    if (loadingTimer !== undefined) clearTimeout(loadingTimer);
    loadingTimer = undefined;
  };
  if (initial === 'deferred') {
    loadingTimer = setTimeout(() => {
      loadingTimer = undefined;
      paint();
    }, 150);
  } else if (initial === 'immediate') paint();
  return {
    publish: () => {
      if (!isCurrent() || !isReady() || frame !== undefined) return;
      clearLoadingTimer();
      frame = requestAnimationFrame(() => { frame = undefined; paint(); });
    },
    finish: () => {
      clearLoadingTimer();
      if (frame !== undefined) cancelAnimationFrame(frame);
      frame = undefined;
      paint();
    },
  };
}
