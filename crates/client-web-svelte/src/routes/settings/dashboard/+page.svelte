<script lang="ts">
  // Dashboard placeholder. The vanilla client's renderMetadataDashboard() is a
  // real view with filters; out of scope for the PoC. Linked from the settings
  // nav so the section isn't a dead end.
</script>

<section class="panel page-panel">
  <h3>Dashboard</h3>
  <p class="muted">
    Metadata dashboard would live here (port of <code>renderMetadataDashboard()</code>).
    See the <a href="/settings/logs">Logs</a> section for a fully ported example.
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
