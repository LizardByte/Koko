<script lang="ts">
  // AuthShell — replaces renderAuthShell() (../client-web/src/app/auth.ts:59-78).
  // Brand mark uses the Koko logo (imported as a Vite URL asset — see import
  // below). Error panel reads from the ui store (the vanilla client reads
  // state.error).
  import { ui } from '$lib/stores';
  import type { Snippet } from 'svelte';
  // Import as a Vite URL asset so it resolves in both SvelteKit (served from
  // the app origin) and Storybook (served from the Storybook dev/build origin).
  // Plain `*.svg` imports resolve to a hashed URL string at build time
  // (see vite/client.d.ts `declare module '*.svg'`), working in both contexts.
  import KokoLogo from '$lib/assets/Koko.svg';

  type Props = {
    title: string;
    description: string;
    children: Snippet;
  };
  let { title, description, children }: Props = $props();
</script>

<div class="auth-shell">
  <section class="auth-panel panel">
    <div class="auth-header">
      <div class="brand-mark logo-brand-mark">
        <img class="brand-logo" src={KokoLogo} alt="" />
      </div>
      <div>
        <h1>Koko</h1>
        <p class="muted">{description}</p>
      </div>
    </div>
    <div class="auth-copy">
      <h2>{title}</h2>
    </div>
    {#if ui.error}
      <section class="panel error-panel auth-error-panel">{ui.error}</section>
    {/if}
    {@render children()}
  </section>
</div>

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
  .auth-header h1,
  .auth-copy h2 {
    margin: 0;
  }
  .auth-error-panel {
    margin-bottom: 1rem;
  }
</style>
