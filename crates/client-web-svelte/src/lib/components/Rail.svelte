<script lang="ts">
  // Rail — replaces renderRail() (../client-web/src/app.ts:368-426). The
  // persistent sidebar: brand block, Home + per-library nav buttons (with
  // refresh rings driven by metadata_refresh_pending), user card, Settings,
  // Sign out. Collapses to 88px on item pages (driven by the `collapsed`
  // prop from the layout).
  //
  // The libraries list is a prop (not a direct store read) so stories and
  // tests can pass fixtures without seeding the global store. Production
  // callers pass `libraries.libraries` from the store; the store is NOT
  // imported here. Auth + routing remain store reads (truly global app state
  // that doesn't vary per Rail instance).
  import Icon from './Icon.svelte';
  import UserAvatar from './UserAvatar.svelte';
  import { selectedLibraryIcon } from '$lib/ui';
  import { auth } from '$lib/stores';
  import { isMockApi } from '$lib/api';
  import { page } from '$app/state';
  import { goto } from '$app/navigation';
  import KokoLogo from '$lib/assets/Koko.svg';
  import { navRegion, navigateList } from '$lib/actions/navRegion';
  import type { MediaLibrary } from '$lib/api';

  type Props = {
    collapsed?: boolean;
    /** Libraries to show in the nav. Defaults to empty (caller passes the
     *  store's list in production; stories pass fixtures). */
    libraries?: MediaLibrary[];
  };
  let { collapsed = false, libraries: libraryList = [] }: Props = $props();

  const mock = $derived(isMockApi());

  // Which nav target is active — derived from the current path. Home is
  // active only on the all-libraries home ('/'); a specific library's page
  // lights up that library's button instead (vanilla app.ts:396 likewise
  // gates Home on activeLibraryId() being undefined).
  const onHome = $derived(page.url.pathname === '/');
  const onSettings = $derived(page.url.pathname.startsWith('/settings'));
  const activeLibraryId = $derived(
    page.url.pathname.startsWith('/libraries/') ? Number(page.url.pathname.split('/')[2]) : undefined,
  );

  /** Refresh ring percent for a library, or undefined when no refresh is active. */
  function refreshPercent(library: MediaLibrary): number | undefined {
    // Matches vanilla libraryRefreshProgress() (activities.ts:42-60): the ring
    // only shows when there is an active metadata-refresh activity OR the
    // library has both total>0 and pending>0 stored progress. The active-
    // activity path needs the activities store wired into the rail (Phase 6);
    // until then the stored-progress clause alone reproduces the seed-data
    // behavior (Movies/Shows: total>0 pending=0 → no ring; Music: total=0).
    if (library.metadata_refresh_total <= 0 || library.metadata_refresh_pending <= 0) {
      return undefined;
    }
    const ratio = library.metadata_refresh_completed / library.metadata_refresh_total;
    return Math.round(ratio * 100);
  }

  function navHome() {
    goto('/');
  }

  function navLibrary(id: number) {
    goto(`/libraries/${id}`);
  }

  function navSettings() {
    goto('/settings');
  }

  async function signOut() {
    auth.logout();
    await goto('/login');
  }

  // Element refs for navigation region.
  let homeButtonEl = $state<HTMLButtonElement | undefined>(undefined);
  let navRail = $state<HTMLElement | undefined>(undefined);
</script>

<aside class="library-rail" class:collapsed use:navRegion={{
  name: 'sidebar',
  navigate: (direction, current) => {
    // Sidebar is a vertical list — handle up/down internally.
    if (direction === 'up' || direction === 'down') {
      return navigateList(direction, current, navRail ?? current.parentElement!);
    }
    // Left/right → delegate to global engine (transition to content).
    return false;
  },
  enter: {
    // When entering sidebar from content (left direction), always go to Home.
    left: () => homeButtonEl,
    right: () => homeButtonEl,
  },
}}>
  <div class="library-rail-top">
    <div class="brand-block">
      <div class="brand-mark logo-brand-mark">
        <img class="brand-logo" src={KokoLogo} alt="" />
      </div>
      <div>
        <h1>Koko</h1>
        {#if mock}<p>Mock data</p>{/if}
      </div>
    </div>

    <nav class="rail-nav" bind:this={navRail}>
      <button type="button" class="rail-button" class:active={onHome} onclick={navHome} bind:this={homeButtonEl}>
        <span class="rail-icon"><Icon name="house" size={18} /></span>
        <span class="rail-label">Home</span>
      </button>
      {#each libraryList as library (library.id)}
        <button
          type="button"
          class="rail-button"
          class:active={activeLibraryId === library.id}
          title={library.name}
          onclick={() => navLibrary(library.id)}
        >
          <span class="rail-icon"><Icon name={selectedLibraryIcon(library.kind)} size={18} /></span>
          <span class="rail-library-copy">
            <span class="rail-label">{library.name}</span>
            {#if refreshPercent(library) !== undefined}
              <span
                class="library-refresh-indicator"
                title="Metadata refresh progress: {library.metadata_refresh_completed}/{library.metadata_refresh_total}"
                aria-label="Metadata refresh in progress"
              >
                <span
                  class="library-refresh-ring"
                  style="--library-refresh-progress: {refreshPercent(library)}%"
                ></span>
              </span>
            {/if}
          </span>
        </button>
      {/each}
    </nav>
  </div>

  <div class="library-rail-bottom">
    {#if auth.currentUser}
      <div class="rail-user-card">
        <UserAvatar user={auth.currentUser} class="rail-avatar" />
        <div class="rail-user-copy">
          <strong>{auth.currentUser.username}</strong>
          <span>{auth.currentUser.admin ? 'Administrator' : 'Signed in'}</span>
        </div>
      </div>
    {/if}
    <button type="button" class="rail-button rail-settings" class:active={onSettings} onclick={navSettings}>
      <span class="rail-icon"><Icon name="settings" size={18} /></span>
      <span class="rail-label">Settings</span>
    </button>
    <button type="button" class="rail-button" onclick={signOut}>
      <span class="rail-icon"><Icon name="log-out" size={18} /></span>
      <span class="rail-label">Sign out</span>
    </button>
  </div>
</aside>

<style>
  /*
   * All rail-specific rules are scoped here — Rail is the sole owner of
   * .rail-button, .rail-label, .rail-settings, .library-refresh-*, and the
   * .rail-user-card family. Promoted out of app.css so they're co-located
   * with their only emitter (and SonarCloud's CSS analyzer skips .svelte
   * files, eliminating false-positive contrast warnings like the one that
   * fired on .rail-button:hover color:#fff). The collapsed-rail layout
   * overrides (.library-rail.collapsed …) stay global in app.css since they
   * target a parent container state.
   */
  strong {
    color: var(--color-text-primary);
    font-size: 0.85rem;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  :global(.rail-button) {
    display: flex;
    align-items: center;
    justify-content: flex-start;
    gap: 0.7rem;
    width: 100%;
    padding: 0.85rem 0.9rem;
    border-radius: 16px;
    background: transparent;
    box-shadow: none;
    color: #b6c4d8;
    text-align: left;
    text-decoration: none;
  }

  :global(.rail-button.active),
  :global(.rail-button:hover) {
    background: var(--surface-4);
    color: var(--color-text-primary);
  }

  :global(.rail-settings) {
    width: 100%;
  }

  :global(.rail-label) {
    max-width: 100%;
    font-size: 0.92rem;
    line-height: 1.2;
    text-align: left;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  :global(.library-refresh-indicator) {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 1.12rem;
    height: 1.12rem;
    flex: 0 0 auto;
  }

  :global(.library-refresh-ring) {
    position: relative;
    width: 100%;
    height: 100%;
    border-radius: 999px;
    background: conic-gradient(
      var(--color-brand-blue) var(--library-refresh-progress, 0%),
      rgba(255, 255, 255, 0.14) 0
    );
  }

  :global(.library-refresh-ring)::after {
    content: '';
    position: absolute;
    inset: 2px;
    border-radius: inherit;
    background: rgba(14, 20, 35, 0.96);
  }

  :global(.rail-user-card) {
    display: flex;
    gap: 0.7rem;
    align-items: center;
    padding: 0.85rem 0.9rem;
    border-radius: 16px;
    background: var(--surface-2);
    color: var(--color-text-secondary);
  }

  :global(.rail-user-card span) {
    color: var(--color-text-muted);
    font-size: 0.82rem;
  }

  :global(.rail-user-copy) {
    display: flex;
    flex-direction: column;
    gap: 0.15rem;
    min-width: 0;
  }

  :global(.rail-library-copy) {
    display: inline-flex;
    align-items: center;
    gap: 0.55rem;
    min-width: 0;
  }

  @media (max-width: 960px) {
    :global(.rail-button) {
      min-width: 110px;
    }
  }
</style>
