<script lang="ts">
  // Global Storybook decorator: seeds the store singletons + mock $app/state
  // before rendering a story, and resets on cleanup. Registered in preview.ts.
  //
  // Stories select a fixture preset via the `preset` arg (default 'empty'):
  //   <Story name="Movie" args={{ preset: 'item-movie' }} />
  // Stories can also override the mock route pathname via `route`:
  //   <Story name="OnItemPage" args={{ preset: 'item-movie', route: '/items/101' }} />
  import type { Snippet } from 'svelte';
  import { applyPreset, resetStores, type Preset } from '$lib/storybook/presets';
  import { setPage } from '$lib/storybook/mockAppState.svelte';

  let { preset = 'empty', route, children }: { preset?: Preset; route?: string; children: Snippet } = $props();

  // (Re)apply whenever the preset/route args change.
  $effect(() => {
    applyPreset(preset);
    if (route) setPage({ pathname: route });
  });

  // Reset on teardown so the next story starts clean.
  $effect(() => {
    return () => resetStores();
  });
</script>

{@render children()}
