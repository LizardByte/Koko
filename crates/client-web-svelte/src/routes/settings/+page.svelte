<script lang="ts">
  // Settings index (the "General" landing). Stub — the vanilla client's
  // settingsView.ts builds a large dynamic form from settingsResponse; porting
  // that is moderate effort (see PROPOSAL.md §4) and not needed to demonstrate
  // the architecture. This page confirms the /settings route resolves.
</script>

<section class="panel page-panel">
  <h3>General</h3>
  <p class="muted">
    General settings form would live here (port of <code>settingsView.ts</code>'s
    General section). Select another section from the left to see working ports —
    in particular <a href="/settings/logs">Logs</a>.
  </p>
</section>

<style>
  .panel {
    background: var(--koko-surface, #fff);
    border: 1px solid var(--koko-border, #ddd);
    border-radius: 8px;
    padding: 1rem 1.25rem;
  }
  h3 {
    margin: 0 0 0.5rem;
  }
  .muted {
    color: var(--koko-muted, #777);
    font-size: 0.9rem;
    line-height: 1.5;
  }
  a {
    color: #2563eb;
  }
  code {
    background: rgba(127, 127, 127, 0.15);
    padding: 0.05rem 0.3rem;
    border-radius: 3px;
    font-size: 0.85rem;
  }
</style>
