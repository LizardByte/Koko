<script lang="ts">
  // HomeNavbar — replaces renderHomeNavbar() + renderHomeTabs()
  // (../client-web/src/app/homeView.ts:655-677, 976-1014). The 5 browse tabs,
  // a search form whose toggle button flips between submit (Search) and button
  // (Clear search), and the per-library scan + refresh-metadata action buttons
  // (only shown when a library is active).
  import Icon from './Icon.svelte';
  import { HOME_TABS } from '$lib';
  import { catalog, libraries, ui } from '$lib/stores';

  let searchInput = $state('');
  // Debounce timer for live-search-on-type (vanilla eventBindings.ts:572-583
  // uses 250ms). Stored on the component so each keystroke resets it.
  let searchDebounce: ReturnType<typeof setTimeout> | undefined;

  // Keep the input in sync with the store value (e.g. cleared elsewhere).
  $effect(() => {
    searchInput = catalog.searchQuery;
  });

  const activeLibrary = $derived(
    catalog.activeLibraryId !== undefined ? libraries.byId(catalog.activeLibraryId) : undefined,
  );

  function selectTab(tabId: (typeof HOME_TABS)[number]['id']) {
    // Matches vanilla eventBindings.ts:610-644: switching tabs clears any
    // active search and browse filter so the selected tab shows cleanly.
    catalog.homeTab = tabId;
    catalog.clearSearch();
    searchInput = '';
  }

  async function submitSearch(event: SubmitEvent) {
    event.preventDefault();
    if (searchDebounce) clearTimeout(searchDebounce);
    await catalog.runSearch(searchInput);
  }

  // Live search as the user types — vanilla binds an input handler with a
  // 250ms debounce (eventBindings.ts:572-583). Submit still works for IME /
  // explicit "enter" and runs immediately.
  function onSearchInput(event: Event) {
    searchInput = (event.currentTarget as HTMLInputElement).value;
    if (searchDebounce) clearTimeout(searchDebounce);
    searchDebounce = setTimeout(() => {
      void catalog.runSearch(searchInput);
    }, 250);
  }

  async function clearSearch() {
    if (searchDebounce) clearTimeout(searchDebounce);
    searchInput = '';
    catalog.clearSearch();
  }

  async function scanActiveLibrary() {
    if (activeLibrary) {
      ui.setError(undefined);
      try {
        await libraries.scan(activeLibrary.id);
      } catch (err) {
        ui.setError(err instanceof Error ? err.message : String(err));
      }
    }
  }

  async function refreshActiveLibraryMetadata() {
    if (activeLibrary) {
      ui.setError(undefined);
      try {
        await libraries.refreshMetadata(activeLibrary.id);
      } catch (err) {
        ui.setError(err instanceof Error ? err.message : String(err));
      }
    }
  }
</script>

<header class="home-navbar">
  <nav class="browse-tabs" aria-label="Browse views">
    {#each HOME_TABS as tab (tab.id)}
      <button
        type="button"
        class="browse-tab-button"
        class:active={catalog.homeTab === tab.id}
        onclick={() => selectTab(tab.id)}
      >
        {tab.label}
      </button>
    {/each}
  </nav>
  <div class="home-navbar-tools">
    <form class="search-form" onsubmit={submitSearch}>
      <input
        type="search"
        name="search"
        placeholder="Search"
        autocomplete="off"
        bind:value={searchInput}
        oninput={onSearchInput}
        aria-label="Search"
      />
      {#if catalog.searchQuery}
        <button
          type="button"
          class="icon-button search-toggle-button"
          title="Clear search"
          aria-label="Clear search"
          onclick={clearSearch}
        >
          <Icon name="x" size={18} />
        </button>
      {:else}
        <button
          type="submit"
          class="icon-button search-toggle-button"
          title="Search"
          aria-label="Search"
        >
          <Icon name="search" size={18} />
        </button>
      {/if}
    </form>
    {#if activeLibrary}
      <button
        type="button"
        class="icon-button secondary-button"
        title="Scan library"
        onclick={scanActiveLibrary}
      >
        <Icon name="folder-sync" size={18} />
      </button>
      <button
        type="button"
        class="icon-button secondary-button"
        title="Refresh metadata"
        onclick={refreshActiveLibraryMetadata}
      >
        <Icon name="database-zap" size={18} />
      </button>
    {/if}
  </div>
</header>

<!--
  Render the full-page search popover/results when a query is active. Kept here
  for cohesion; the actual results rendering is delegated to the home page's
  search section so the navbar stays focused.
-->

<style>
  /*
   * Component-owned (HomeNavbar-only). Values mirror vanilla style.css
   * :578-715. .icon-button is shared (app.css). The 960px responsive override
   * is here because only HomeNavbar renders .home-navbar.
   */
  .home-navbar {
    position: sticky;
    top: 0;
    z-index: 12;
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 0.75rem;
    width: 100%;
    min-height: 3.75rem;
    padding: 0.45rem 0.9rem;
    border-bottom: 1px solid rgba(255, 255, 255, 0.08);
    background: rgba(7, 11, 20, 0.96);
    backdrop-filter: blur(18px);
  }

  .home-navbar-tools {
    display: flex;
    align-items: center;
    justify-content: flex-end;
    gap: 0.45rem;
    min-width: 0;
  }

  .browse-tabs {
    display: flex;
    gap: 0.25rem;
    padding: 0;
    overflow-x: auto;
  }

  .browse-tab-button {
    flex: 0 0 auto;
    background: transparent;
    box-shadow: none;
    color: #9ab1d1;
    min-height: 2.35rem;
    padding: 0 0.7rem;
  }

  .browse-tab-button.active,
  .browse-tab-button:hover {
    background: rgba(255, 255, 255, 0.08);
    color: #fff;
  }

  .search-form {
    display: flex;
    width: min(100%, 360px);
    align-items: stretch;
  }

  .search-form input[type='search'] {
    height: 2.75rem;
    min-height: 2.75rem;
    border-top-right-radius: 0;
    border-bottom-right-radius: 0;
    border-right: 0;
  }

  .search-form input[type='search']::-webkit-search-cancel-button {
    appearance: none;
  }

  .search-toggle-button {
    width: 2.75rem;
    height: 2.75rem;
    min-height: 2.75rem;
    border-top-left-radius: 0;
    border-bottom-left-radius: 0;
    box-shadow: none;
  }

  @media (max-width: 960px) {
    .home-navbar {
      flex-wrap: wrap;
    }
    .home-navbar-tools {
      width: 100%;
    }
    .search-form {
      width: 100%;
    }
  }
</style>
