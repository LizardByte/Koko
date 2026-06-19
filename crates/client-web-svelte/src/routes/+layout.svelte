<script lang="ts">
  // App shell + auth guard. Replaces the bootstrap logic in app.ts's startApp()
  // and the chrome from renderPageNavbar() in ui.ts. In mock mode the seeded
  // user is already "logged in" via getAppBootstrap(), so the guard passes.
  import '../app.css';
  import { onMount } from 'svelte';
  import { page } from '$app/state';
  import { goto } from '$app/navigation';
  import { auth } from '$lib/auth.svelte';
  import { isMockApi } from '$lib/api';
  import Icon from '$lib/components/Icon.svelte';
  import Spinner from '$lib/components/Spinner.svelte';

  let { children } = $props();
  const mock = $derived(isMockApi());

  onMount(() => {
    auth.init();
  });

  // The login page renders without auth; everything else is guarded.
  const isLoginRoute = $derived(page.url.pathname.startsWith('/login'));
  const showShell = $derived(!auth.loading && (auth.isLoggedIn || isLoginRoute));

  // If bootstrap finished, user isn't logged in, and we're not already on
  // /login, redirect there. (SvelteKit goto from a derived effect.)
  $effect(() => {
    if (!auth.loading && !auth.isLoggedIn && !isLoginRoute) {
      goto('/login');
    }
  });

  async function logout() {
    auth.logout();
  }
</script>

{#if auth.loading}
  <div class="full-center"><Spinner label="Starting Koko…" /></div>
{:else if showShell}
  <div class="app-shell">
    <header class="page-navbar">
      <a class="brand" href="/"><Icon name="house" size={20} /> Koko</a>
      <nav>
        <a href="/" class:active={page.url.pathname === '/'}>Home</a>
        <a href="/settings" class:active={page.url.pathname.startsWith('/settings')}>Settings</a>
      </nav>
      <div class="navbar-spacer"></div>
      {#if mock}
        <span class="mock-badge">MOCK API</span>
      {/if}
      {#if auth.currentUser}
        <div class="user-menu">
          <span class="user-name">{auth.currentUser.username}</span>
          {#if auth.currentUser.admin}<span class="admin-pill">admin</span>{/if}
          <button class="logout-btn" onclick={logout} title="Sign out" aria-label="Sign out">
            <Icon name="log-out" size={16} />
          </button>
        </div>
      {:else}
        <a class="signin-link" href="/login"><Icon name="log-in" size={16} /> Sign in</a>
      {/if}
    </header>
    <main>
      {@render children()}
    </main>
  </div>
{:else}
  <!-- Not logged in and not on /login: the $effect above redirects to /login. -->
  <div class="full-center"><Spinner label="Redirecting to sign in…" /></div>
{/if}

<style>
  .app-shell {
    min-height: 100vh;
  }
  .page-navbar {
    position: sticky;
    top: 0;
    z-index: 10;
    display: flex;
    align-items: center;
    gap: 1rem;
    padding: 0.7rem 1.5rem;
    border-bottom: 1px solid var(--koko-border, #ddd);
    background: var(--koko-surface, #fff);
  }
  .brand {
    display: inline-flex;
    align-items: center;
    gap: 0.4rem;
    font-weight: 700;
    color: inherit;
    text-decoration: none;
  }
  nav {
    display: flex;
    gap: 0.8rem;
  }
  nav a {
    color: var(--koko-muted, #777);
    text-decoration: none;
    font-size: 0.9rem;
    padding: 0.2rem 0.2rem;
  }
  nav a.active {
    color: inherit;
    font-weight: 600;
  }
  .navbar-spacer {
    flex: 1;
  }
  .mock-badge {
    font-size: 0.7rem;
    font-weight: 700;
    padding: 0.1rem 0.45rem;
    border-radius: 4px;
    background: rgba(234, 179, 8, 0.2);
    color: #b45309;
  }
  .user-menu {
    display: inline-flex;
    align-items: center;
    gap: 0.5rem;
  }
  .user-name {
    font-size: 0.85rem;
    font-weight: 500;
  }
  .admin-pill {
    font-size: 0.68rem;
    padding: 0.05rem 0.35rem;
    border-radius: 3px;
    background: rgba(59, 130, 246, 0.18);
    color: #1d4ed8;
  }
  .logout-btn {
    display: inline-flex;
    align-items: center;
    border: 1px solid var(--koko-border, #ddd);
    border-radius: 6px;
    background: transparent;
    cursor: pointer;
    padding: 0.3rem;
    color: inherit;
  }
  .logout-btn:hover {
    background: rgba(127, 127, 127, 0.1);
  }
  .signin-link {
    display: inline-flex;
    align-items: center;
    gap: 0.3rem;
    font-size: 0.85rem;
    text-decoration: none;
    color: inherit;
    padding: 0.3rem 0.7rem;
    border-radius: 6px;
    border: 1px solid var(--koko-border, #ddd);
  }
  .full-center {
    min-height: 100vh;
    display: flex;
    align-items: center;
    justify-content: center;
  }
  main {
    padding: 1.5rem;
  }
</style>
