// Global Storybook decorator: seeds the store singletons + mock $app/state
// around every story based on `args.preset` / `args.route`.
//
// Why a plain function (not a Svelte component)?
//   The Svelte renderer decorator contract (see `decorateStory` in
//   @storybook/svelte) calls each decorator as `decorator(storyFn, context)`
//   where `context.args` holds the story's args. A function decorator is the
//   documented way to read story context for mocking (addon-svelte-csf README
//   "Accessing Story context"). Returning `storyFn()` hands rendering back to
//   the renderer's normal mount path.
//
//   Registering a Svelte *component* as the decorator instead (the earlier
//   `[WithStores]` / `() => WithStores` attempts) made the renderer invoke the
//   component function with the wrong `(internal, props)` shape, corrupting
//   Svelte 5's internal mount and producing `anchor.before is not a function`.
//
// Why untrack + synchronous seeding?
//   The Svelte renderer invokes decorators from inside a reactive effect
//   (decorateStory's reduce). Mutating store `$state` synchronously from there
//   trips Svelte 5's `state_unsafe_mutation` guard ("Updating state inside a
//   $derived/template expression is forbidden") *and* would register reactive
//   dependencies we don't want.
//
//   `untrack` runs the seeding outside the current reactive computation, so
//   the mutation is permitted and no spurious dependencies are recorded. It
//   stays synchronous, so the component mounted by `storyFn()` below sees the
//   already-seeded stores on first render (no race).
//
// `applyPreset` always calls `resetStores()` first, so state never bleeds
// between stories — no `$effect` cleanup is required.
import { untrack } from 'svelte';
import type { Decorator } from '@storybook/svelte';
import { applyPreset, type Preset } from '$lib/storybook/presets';
import { setPage } from '$lib/storybook/mockAppState.svelte';

export const withStores: Decorator = (storyFn, context) => {
  const args = (context.args ?? {}) as { preset?: Preset; route?: string };
  untrack(() => {
    applyPreset(args.preset ?? 'empty');
    if (args.route) setPage({ pathname: args.route });
  });
  return storyFn();
};
