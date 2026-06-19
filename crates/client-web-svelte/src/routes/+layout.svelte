<script lang="ts">
  // App shell. Mirrors the vanilla client's chrome (page navbar + mock-mode
 // badge). Real migration would lift renderPageNavbar() from ui.ts into a
  // <Navbar /> component.
  import '../app.css';
  import { isMockApi } from '$lib/api';

  let { children } = $props();
  const mock = $derived(isMockApi());
</script>

<div class="app-shell">
  <header class="page-navbar">
    <a class="brand" href="/">Koko <span class="muted">— Svelte PoC</span></a>
    <nav>
      <a href="/">Home</a>
      <a href="/settings/logs">Settings → Logs</a>
    </nav>
    {#if mock}
      <span class="tag warning mock-badge">MOCK API</span>
    {/if}
  </header>
  <main>
    {@render children()}
  </main>
</div>

<style>
  .app-shell {
    min-height: 100vh;
  }
  .page-navbar {
    display: flex;
    align-items: center;
    gap: 1.5rem;
    padding: 0.75rem 1.5rem;
    border-bottom: 1px solid var(--koko-border, #ddd);
    background: var(--koko-surface, #fff);
  }
  .page-navbar nav {
    display: flex;
    gap: 1rem;
  }
  .page-navbar a {
    color: inherit;
    text-decoration: none;
  }
  .page-navbar a:hover {
    text-decoration: underline;
  }
  .brand {
    font-weight: 600;
  }
  .mock-badge {
    margin-left: auto;
  }
  .muted {
    color: var(--koko-muted, #777);
  }
  main {
    padding: 1.5rem;
  }
</style>
