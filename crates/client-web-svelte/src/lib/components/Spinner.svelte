<script lang="ts">
  // Stand-in for the vanilla client's spinner/sync helper. Pure CSS, no JS.
  type Props = { label?: string };
  let { label = 'Loading…' }: Props = $props();
</script>

<div class="spinner-row" role="status" aria-live="polite">
  <span class="spinner" aria-hidden="true"></span>
  <span class="muted">{label}</span>
</div>

<style>
  .spinner-row {
    display: inline-flex;
    align-items: center;
    gap: 0.5rem;
  }
  .spinner {
    width: 14px;
    height: 14px;
    border: 2px solid var(--koko-border, #ccc);
    border-top-color: currentColor;
    border-radius: 50%;
    animation: spin 0.7s linear infinite;
  }
  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }
  .muted {
    color: var(--koko-muted, #777);
    font-size: 0.85rem;
  }
</style>
