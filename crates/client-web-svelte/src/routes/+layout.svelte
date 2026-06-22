<script lang="ts">
  // Root layout — replaces the render() orchestration in app.ts:428-483 and the
  // auth gating in startApp(). Shows the auth screens when bootstrap requires
  // setup/login; otherwise renders the rail + page backdrop + current page.
  import '../app.css';
  import { onMount } from 'svelte';
  import { beforeNavigate } from '$app/navigation';
  import { page } from '$app/state';
  import { goto } from '$app/navigation';
  import { auth, ui, libraries, activities, playback } from '$lib/stores';
  import { noop } from '$lib/constants';
  import Rail from '$lib/components/Rail.svelte';
  import LoginScreen from '$lib/components/LoginScreen.svelte';
  import WelcomeScreen from '$lib/components/WelcomeScreen.svelte';
  import PlayerOverlay from '$lib/components/player/PlayerOverlay.svelte';
  import ControlsHelp from '$lib/components/player/ControlsHelp.svelte';
  import { spatialNavigation } from '$lib/actions/spatialNavigation';
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

  // Load system activities once logged in (needed to detect in-progress
  // metadata/scan work for the polling predicate).
  $effect(() => {
    if (auth.isLoggedIn && !activities.systemActivities) {
      activities.loadActivities().catch(noop);
    }
  });

  // Auto-refresh polling (Phase 6.5d). When metadata refresh or library scan
  // activities are in-progress, poll every 1500ms to update the UI as they
  // complete. The $effect reactively arms/tears-down the timer based on
  // activities.shouldPoll — when work finishes, the predicate goes false and
  // the cleanup cancels the timer. Mirrors vanilla app.ts:245-255, 724-840
  // (simplified — Svelte reactivity replaces snapshot-diff + force-render).
  $effect(() => {
    if (!auth.isLoggedIn || !activities.shouldPoll) return;
    let timer: ReturnType<typeof setTimeout>;
    const tick = () => {
      activities.poll().finally(() => {
        // Re-arm only if still polling (the predicate is reactive — if it
        // flipped to false, this $effect's cleanup will have cancelled).
        if (activities.shouldPoll) {
          timer = setTimeout(tick, 1500);
        }
      });
    };
    timer = setTimeout(tick, 1500);
    return () => clearTimeout(timer);
  });

  // Never-scanned library polling (Phase 7 minor gap). When any library has
  // status 'never_scanned', poll every 1800ms to detect when the initial scan
  // starts. Mirrors vanilla shouldAutoRefreshLibraries (app.ts:172-175).
  $effect(() => {
    if (!auth.isLoggedIn || !activities.shouldPollLibraries) return;
    let timer: ReturnType<typeof setTimeout>;
    const tick = () => {
      libraries.load().finally(() => {
        if (activities.shouldPollLibraries) {
          timer = setTimeout(tick, 1800);
        }
      });
    };
    timer = setTimeout(tick, 1800);
    return () => clearTimeout(timer);
  });

  // Opportunity H: intercept back-button when the player overlay is open.
  // When the user pressed back (popstate) and a player state was pushed,
  // close the player instead of navigating away.
  beforeNavigate(({ type }) => {
    if (type === 'leave') return;
    // If the player is open and this is a popstate (back button), close it.
    if (playback.isOpen && type === 'popstate') {
      if (playback.mode === 'media') {
        playback.close();
      } else if (playback.mode === 'trailer') {
        playback.closeTrailer();
      }
      // Don't cancel — let the back navigation proceed (it undoes our pushState).
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
  <div class="app-shell" class:rail-collapsed={railCollapsed} use:spatialNavigation>
    <div class="page-backdrop"></div>
    <Rail collapsed={railCollapsed} libraries={libraries.libraries} />
    <div class="main-shell">
      <div class="main-shell-inner">
        {#if ui.error}
          <section class="panel error-panel page-panel">{ui.error}</section>
        {/if}
        {@render children()}
      </div>
    </div>
  </div>
  <!-- Player overlay — rendered above the app shell when playback is active -->
  <PlayerOverlay />
  <!-- Controls help — triggered by Select button / "?" key -->
  <ControlsHelp isOpen={ui.controlsHelpOpen} onclose={() => (ui.controlsHelpOpen = false)} />
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
