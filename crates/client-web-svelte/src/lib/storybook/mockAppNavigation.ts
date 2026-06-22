// Storybook stub for $app/navigation. Aliased via .storybook/main.ts.
// goto() is a no-op in stories (there's no router to navigate); navigating
// is inert. Stories that need to assert navigation can spy on goto.

// `noop` body avoids S1186 (empty function) while keeping these inert.
function noop(): void {}

export function goto(): Promise<void> {
  return Promise.resolve();
}

export function invalidate(): Promise<void> {
  return Promise.resolve();
}

export function invalidateAll(): Promise<void> {
  return Promise.resolve();
}

export function pushState(): void { noop(); }
export function replaceState(): void { noop(); }

export function beforeNavigate(): void { noop(); }
export function afterNavigate(): void { noop(); }

export const navigating = { from: null, to: null, type: null };
