<script lang="ts">
  // Settings sub-nav. Replaces the section list in renderSettingsPage()
  // (../client-web/src/app/settingsView.ts). SvelteKit nested routes handle
  // the active state via $page; the vanilla client hand-rolled this.
  import { page } from '$app/state';
  import Icon from '$lib/components/Icon.svelte';

  let { children } = $props();

  const sections = [
    { id: '', label: 'General', icon: 'settings' },
    { id: 'libraries', label: 'Libraries', icon: 'database' },
    { id: 'dashboard', label: 'Dashboard', icon: 'layers' },
    { id: 'logs', label: 'Logs', icon: 'film' },
  ];

  function isActive(id: string): boolean {
    const path = page.url.pathname.replace(/\/$/, '');
    if (id === '') {
      return path === '/settings';
    }
    return path === `/settings/${id}`;
  }
</script>

<div class="settings-layout">
  <aside class="settings-nav">
    <h2>Settings</h2>
    <nav>
      {#each sections as section (section.id)}
        <a href="/settings/{section.id}" class:active={isActive(section.id)}>
          <Icon name={section.icon} size={16} />
          {section.label}
        </a>
      {/each}
    </nav>
  </aside>
  <div class="settings-content">
    {@render children()}
  </div>
</div>

<style>
  .settings-layout {
    display: grid;
    grid-template-columns: 220px 1fr;
    gap: 1.5rem;
    align-items: start;
  }
  @media (max-width: 700px) {
    .settings-layout {
      grid-template-columns: 1fr;
    }
  }
  .settings-nav {
    position: sticky;
    top: 70px;
  }
  .settings-nav h2 {
    font-size: 1.1rem;
    margin: 0 0 0.8rem;
  }
  .settings-nav nav {
    display: flex;
    flex-direction: column;
    gap: 0.15rem;
  }
  .settings-nav a {
    display: inline-flex;
    align-items: center;
    gap: 0.5rem;
    padding: 0.45rem 0.6rem;
    border-radius: 6px;
    text-decoration: none;
    color: var(--koko-muted, #777);
    font-size: 0.9rem;
  }
  .settings-nav a:hover {
    background: rgba(127, 127, 127, 0.08);
    color: inherit;
  }
  .settings-nav a.active {
    background: rgba(37, 99, 235, 0.1);
    color: #2563eb;
    font-weight: 600;
  }
</style>
