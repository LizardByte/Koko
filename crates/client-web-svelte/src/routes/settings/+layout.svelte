<script lang="ts">
  // Settings layout — loads settings on mount, renders the SettingsShell
  // (navbar + section nav) around the active section's content.
  import { onMount } from 'svelte';
  import SettingsShell from '$lib/components/settings/SettingsShell.svelte';
  import { settings } from '$lib/stores';

  let { children } = $props();

  onMount(() => {
    if (!settings.response) {
      settings.load().catch(() => {});
    }
  });
</script>

<SettingsShell>
  {@render children()}
</SettingsShell>
