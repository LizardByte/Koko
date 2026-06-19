<script lang="ts">
  // Mirrors the vanilla client's `.tag` / `.tag.warning` / `.tag.danger-tag`
  // classes (see style.css). Variants map 1:1.
  type Props = {
    variant?: 'default' | 'warning' | 'danger' | 'info';
    children: import('svelte').Snippet;
  };
  let { variant = 'default', children }: Props = $props();
</script>

<span class="tag {variant}">{@render children()}</span>

<style>
  .tag {
    display: inline-block;
    padding: 0.1rem 0.4rem;
    border-radius: 4px;
    font-size: 0.78rem;
    background: rgba(127, 127, 127, 0.18);
  }
  .warning {
    background: rgba(234, 179, 8, 0.18);
    color: #b45309;
  }
  .danger {
    background: rgba(239, 68, 68, 0.18);
    color: #b91c1c;
  }
  .info {
    background: rgba(59, 130, 246, 0.18);
    color: #1d4ed8;
  }
</style>
