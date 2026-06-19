// Storybook stub for $app/navigation. Aliased via .storybook/main.ts.
// goto() is a no-op in stories (there's no router to navigate); navigating
// is inert. Stories that need to assert navigation can spy on goto.

export function goto(): Promise<void> {
  return Promise.resolve();
}

export function invalidate(): Promise<void> {
  return Promise.resolve();
}

export function invalidateAll(): Promise<void> {
  return Promise.resolve();
}

export function pushState(): void {}
export function replaceState(): void {}

export function beforeNavigate(): void {}
export function afterNavigate(): void {}

export const navigating = { from: null, to: null, type: null };
