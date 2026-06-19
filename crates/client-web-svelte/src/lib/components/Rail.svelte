<script lang="ts">
  // Rail — replaces renderRail() (../client-web/src/app.ts:368-426). The
  // persistent sidebar: brand block, Home + per-library nav buttons (with
  // refresh rings driven by metadata_refresh_pending), user card, Settings,
  // Sign out. Collapses to 88px on item pages (driven by the `collapsed`
  // prop from the layout).
  import Icon from './Icon.svelte';
  import UserAvatar from './UserAvatar.svelte';
  import { selectedLibraryIcon } from '$lib/ui';
  import { auth, libraries } from '$lib/stores';
  import { isMockApi } from '$lib/api';
  import { page } from '$app/state';
  import { goto } from '$app/navigation';

  type Props = { collapsed?: boolean };
  let { collapsed = false }: Props = $props();

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

  function refreshPercent(libraryId: number): number | undefined {
    const library = libraries.byId(libraryId);
    if (!library) {
      return undefined;
    }
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
</script>

<aside class="library-rail" class:collapsed>
  <div class="library-rail-top">
    <div class="brand-block">
      <div class="brand-mark logo-brand-mark">
        <img class="brand-logo" src="/Koko.svg" alt="" />
      </div>
      <div>
        <h1>Koko</h1>
        {#if mock}<p>Mock data</p>{/if}
      </div>
    </div>

    <nav class="rail-nav">
      <button type="button" class="rail-button" class:active={onHome} onclick={navHome}>
        <span class="rail-icon"><Icon name="house" size={18} /></span>
        <span class="rail-label">Home</span>
      </button>
      {#each libraries.libraries as library (library.id)}
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
            {#if refreshPercent(library.id) !== undefined}
              <span
                class="library-refresh-indicator"
                title="Metadata refresh progress: {library.metadata_refresh_completed}/{library.metadata_refresh_total}"
                aria-label="Metadata refresh in progress"
              >
                <span
                  class="library-refresh-ring"
                  style="--library-refresh-progress: {refreshPercent(library.id)}%"
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
   * Component-private rules only. The `.library-refresh-indicator`,
   * `.library-refresh-ring`, and `.rail-settings` rules live in app.css
   * (mirroring vanilla style.css:464-491) — see PORTING_GUIDELINES.md.
   * The scoped `strong` rule below has no vanilla counterpart (the rail
   * user-card label styling is rail-specific glue).
   */
  strong {
    color: #f4f7fb;
    font-size: 0.85rem;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
</style>
