<script lang="ts">
  // Root layout — replaces the render() orchestration in app.ts:428-483 and the
  // auth gating in startApp(). Shows the auth screens when bootstrap requires
  // setup/login; otherwise renders the rail + page backdrop + current page.
  import '../app.css';
  import { onMount } from 'svelte';
  import { page } from '$app/state';
  import { goto } from '$app/navigation';
  import { auth, ui, libraries } from '$lib/stores';
  import Rail from '$lib/components/Rail.svelte';
  import LoginScreen from '$lib/components/LoginScreen.svelte';
  import WelcomeScreen from '$lib/components/WelcomeScreen.svelte';
  import KokoLogo from '$lib/assets/Koko.svg';

  let { children } = $props();

  onMount(() => {
    auth.init();
  });

  // Once bootstrap resolves, gate auth. Redirect to /login if a logged-out
  // user lands on a protected route; redirect to / if logged in on /login.
  const onLoginRoute = $derived(page.url.pathname.startsWith('/login'));
  const isProtected = $derived(!auth.loading && !onLoginRoute);

  $effect(() => {
    if (auth.loading) return;
    if (auth.requiresLogin && isProtected) {
      goto('/login');
    } else if (auth.isLoggedIn && onLoginRoute) {
      goto('/');
    }
  });

  // The rail collapses to 88px on item pages (matches isRailCollapsed in
  // app.ts:312-314 — note vanilla gates on route.page === 'item' only, NOT
  // person pages, so the person page keeps the full-width rail).
  const railCollapsed = $derived(page.url.pathname.startsWith('/items/'));

  // Page backdrop: real backdrop comes from the page (item/home). For now the
  // CSS var defaults to none; pages set it via inline style on .page-backdrop.
  // The libraries store loads in parallel with bootstrap so the rail renders
  // populated immediately once authed.
  $effect(() => {
    if (auth.isLoggedIn && libraries.libraries.length === 0 && !libraries.loading) {
      libraries.load();
    }
  });
</script>

{#if auth.loading}
  <!-- Bootstrap loading shell, same as app.ts:452-456 -->
  <div class="auth-shell">
    <section class="auth-panel panel">
      <div class="auth-header">
        <div class="brand-mark logo-brand-mark">
          <img class="brand-logo" src={KokoLogo} alt="" />
        </div>
        <div>
          <h1>Koko</h1>
          <p class="muted">Checking server state and account access.</p>
        </div>
      </div>
    </section>
  </div>
{:else if auth.requiresSetup}
  <WelcomeScreen />
{:else if auth.requiresLogin && isProtected}
  <!-- Redirecting to /login via $effect; show nothing to avoid flash. -->
{:else if onLoginRoute && !auth.isLoggedIn}
  <LoginScreen />
{:else if auth.isLoggedIn}
  <div class="app-shell" class:rail-collapsed={railCollapsed}>
    <div class="page-backdrop"></div>
    <Rail collapsed={railCollapsed} />
    <div class="main-shell">
      <div class="main-shell-inner">
        {#if ui.error}
          <section class="panel error-panel page-panel">{ui.error}</section>
        {/if}
        {@render children()}
      </div>
    </div>
  </div>
{:else}
  <!-- Fallback during auth transitions. -->
  <div class="auth-shell">
    <p class="muted">Loading…</p>
  </div>
{/if}

<style>
  .auth-shell {
    min-height: 100vh;
    display: grid;
    place-items: center;
    padding: 1.5rem;
  }
  .auth-panel {
    width: min(480px, 100%);
    padding: 1.4rem;
  }
  .auth-header {
    display: flex;
    gap: 1rem;
    margin-bottom: 1rem;
  }
  .auth-header h1 {
    margin: 0;
  }
</style>
