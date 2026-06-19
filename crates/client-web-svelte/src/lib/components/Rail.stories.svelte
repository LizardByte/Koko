<script module>
  // Rail stories. Reads the libraries store + $app/state (page) for active
  // state + auth store for the user card. preset 'home' seeds libraries +
  // current user; the `route` arg simulates which nav is active.
  import { defineMeta } from '@storybook/addon-svelte-csf';
  import Rail from './Rail.svelte';

  const { Story } = defineMeta({
    title: 'Fragments/Rail',
    component: Rail,
    tags: ['autodocs'],
    args: { preset: 'home' },
    parameters: {
      docs: {
        description: {
          component:
            'Persistent sidebar: brand block, Home + per-library nav (with metadata-refresh rings), user card, Settings, Sign out. Collapses to 88px via the `collapsed` prop (item pages). On viewports ≤ 960px it flips to a horizontal bar; the "Full Vertical" story pins the desktop layout so you can review the sidebar without widening the canvas.\n\n' +
            '> ⚠️ **Mostly store-driven.** Only `collapsed` is a real prop; the libraries list, ' +
            'active route, and user card all come from the `libraries` / `auth` stores, seeded via ' +
            'the `preset` / `route` args (see `.storybook/decorators/withStores.ts`). ' +
            'Switch `route` to change which nav item is active.',
        },
      },
    },
  });
</script>

<Story name="Home Active" args={{ preset: 'home', route: '/' }} asChild>
  <Rail />
</Story>

<Story name="Library Active" args={{ preset: 'home', route: '/libraries/2' }} asChild>
  <Rail />
</Story>

<Story name="Collapsed" args={{ preset: 'home', route: '/items/101' }} asChild>
  <Rail collapsed={true} />
</Story>

<!--
  Full Vertical — forces the desktop sidebar layout regardless of the Storybook
  canvas width. The rail normally collapses to a horizontal bar below 960px
  (viewport-based media query), making it hard to review the vertical sidebar
  in a narrow docs/canvas pane. This wrapper pins a fixed-width, fixed-height
  container and resets the small-screen styles so the vertical rail always renders.
-->
<Story name="Full Vertical" args={{ preset: 'home', route: '/' }} asChild>
  <div class="force-vertical-shell">
    <Rail />
  </div>
</Story>

<style>
  .force-vertical-shell {
    /* Fixed desktop-rail dimensions so the rail fills top→bottom regardless
       of the Storybook canvas size. The scoped overrides below reset the
       ≤960px media-query styles that would otherwise flatten it. */
    width: 240px;
    min-width: 240px;
    height: 720px;
    background: #0c111d;
  }

  /* Override the ≤960px media query inside this story so the rail stays
     vertical even when the Storybook canvas is narrow. These mirror the
     desktop (default) library-rail styles from app.css. */
  .force-vertical-shell :global(.library-rail) {
    grid-column: 1;
    grid-row: 1 / -1;
    flex-direction: column;
    align-items: stretch;
    height: 100%;
    max-width: 240px;
    overflow: hidden auto;
    border-right: 1px solid rgba(255, 255, 255, 0.08);
    border-bottom: 0;
  }

  .force-vertical-shell :global(.library-rail-top),
  .force-vertical-shell :global(.library-rail-bottom) {
    flex-direction: column;
    align-items: stretch;
    min-height: 0;
  }

  .force-vertical-shell :global(.rail-nav) {
    flex-direction: column;
    overflow: visible;
  }

  .force-vertical-shell :global(.rail-button) {
    min-width: 0;
  }
</style>
