<script lang="ts">
  // Temporary landing — replaced by Phase 2's home page.
  import { isMockApi } from '$lib/api';
  const mock = $derived(isMockApi());
</script>

<main style="padding:2rem;font-family:Inter,system-ui,sans-serif;color:#f4f7fb">
  <h1>Koko — Svelte port (Phase 0)</h1>
  <p>Foundation laid: shared CSS, full API/types, verbatim mock data, helpers, primitives, stores.</p>
  <p>Mock API: <strong>{mock ? 'on' : 'off'}</strong></p>
  <p>Navigate to /settings/logs once Phase 1+ land.</p>
</main>
