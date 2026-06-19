<script lang="ts">
  // Temporary bootstrap — Phase 1 will replace this with the real rail + guard.
  import '../app.css';
  import { onMount } from 'svelte';
  import { auth } from '$lib/stores';

  let { children } = $props();

  onMount(() => {
    auth.init();
  });
</script>

{#if auth.loading}
  <div style="min-height:100vh;display:grid;place-items:center;color:#9ab1d1;font-family:Inter,system-ui,sans-serif">
    Loading Koko…
  </div>
{:else}
  {@render children()}
{/if}
