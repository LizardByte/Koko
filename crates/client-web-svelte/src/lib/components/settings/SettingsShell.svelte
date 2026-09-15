<script lang="ts">
  // SettingsShell — the settings page chrome: page navbar (title/subtitle/
  // settings_path) + the 6-button section nav sidebar. Port of
  // renderSettingsPage + renderSettingsSectionNav
  // (../client-web/src/app/settingsView.ts:18-38, 663-681).
  //
  // Section nav highlights the active section via $app/state pathname.
  // Content is rendered via the children snippet (the route +page.svelte).
  import { page } from '$app/state';
  import { goto } from '$app/navigation';
  import type { Snippet } from 'svelte';
  import Button from '../Button.svelte';
  import { settings } from '$lib/stores';

  let { children }: { children: Snippet } = $props();

  type Section = { id: string; label: string; path: string };

  // Non-admins see only general + logs (vanilla gates user-management via
  // canManageUsers, but the hub itself is visible to all logged-in users).
  const sections: Section[] = [
    { id: 'general', label: 'General', path: '/settings' },
    { id: 'libraries', label: 'Libraries', path: '/settings/libraries' },
    { id: 'providers', label: 'Providers', path: '/settings/providers' },
    { id: 'scheduled', label: 'Scheduled', path: '/settings/scheduled' },
    { id: 'dashboard', label: 'Dashboard', path: '/settings/dashboard' },
    { id: 'logs', label: 'Logs', path: '/settings/logs' },
  ];

  // Derive active section from the pathname (mirrors activeSettingsSection).
  const activeSection = $derived(
    page.url.pathname === '/settings'
      ? 'general'
      : (page.url.pathname.split('/')[2] ?? 'general'),
  );
</script>

<header class="page-navbar">
  <div>
    <h1>Settings</h1>
    <p class="page-navbar-subtitle">Program configuration</p>
  </div>
  <p class="page-navbar-meta muted">
    {#if settings.settingsPath}Saved to {settings.settingsPath}{/if}
  </p>
</header>

<nav class="settings-section-nav panel page-panel" aria-label="Settings sections">
  {#each sections as section (section.id)}
    <Button
      variant="secondary"
      class={activeSection === section.id ? 'active' : ''}
      label={section.label}
      onclick={() => goto(section.path)}
    />
  {/each}
</nav>

{#if !settings.settings}
  <section class="panel page-panel">
    <div class="empty-state">Settings are still loading…</div>
  </section>
{:else}
  {@render children()}
{/if}
