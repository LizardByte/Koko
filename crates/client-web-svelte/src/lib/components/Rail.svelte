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

  // Which nav target is active — derived from the current path.
  const onHome = $derived(page.url.pathname === '/' || page.url.pathname.startsWith('/libraries'));
  const onSettings = $derived(page.url.pathname.startsWith('/settings'));
  const activeLibraryId = $derived(
    page.url.pathname.startsWith('/libraries/') ? Number(page.url.pathname.split('/')[2]) : undefined,
  );

  function refreshPercent(libraryId: number): number | undefined {
    const library = libraries.byId(libraryId);
    if (!library || library.metadata_refresh_total === 0) {
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
        <span class="rail-icon"><Icon name="layout-grid" size={24} /></span>
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
          <span class="rail-icon"><Icon name={selectedLibraryIcon(library.kind)} size={24} /></span>
          <span class="rail-library-copy">
            <span class="rail-label">{library.name}</span>
            {#if refreshPercent(library.id) !== undefined}
              <span
                class="library-refresh-ring"
                style="--library-refresh-progress: {refreshPercent(library.id)}%"
              ></span>
            {/if}
          </span>
        </button>
      {/each}
    </nav>
  </div>

  <div class="library-rail-bottom">
    {#if auth.currentUser}
      <div class="rail-user-card">
        <UserAvatar user={auth.currentUser} />
        <div class="rail-user-copy">
          <strong>{auth.currentUser.username}</strong>
          <span>{auth.currentUser.admin ? 'Administrator' : 'Signed in'}</span>
        </div>
      </div>
    {/if}
    <button type="button" class="rail-button rail-settings" class:active={onSettings} onclick={navSettings}>
      <span class="rail-icon"><Icon name="settings" size={24} /></span>
      <span class="rail-label">Settings</span>
    </button>
    <button type="button" class="rail-button" onclick={signOut}>
      <span class="rail-icon"><Icon name="log-out" size={24} /></span>
      <span class="rail-label">Sign out</span>
    </button>
  </div>
</aside>

<style>
  .library-refresh-ring {
    position: relative;
    display: inline-block;
    width: 1rem;
    height: 1rem;
    border-radius: 999px;
    background: conic-gradient(
      #5d7bff var(--library-refresh-progress, 0%),
      rgba(255, 255, 255, 0.14) 0
    );
  }

  .library-refresh-ring::after {
    content: '';
    position: absolute;
    inset: 2px;
    border-radius: inherit;
    background: rgba(14, 20, 35, 0.96);
  }

  .rail-settings {
    width: 100%;
  }

  strong {
    color: #f4f7fb;
    font-size: 0.85rem;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
</style>
