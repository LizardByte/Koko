<script lang="ts">
  // CollapsibleText — replaces renderCollapsibleText() (../client-web/src/app/
  // ui.ts:8-24). Uses local store-backed state (ui.expandedTextKeys) so the
  // expand/collapse survives re-renders, matching the vanilla client's
  // data-toggle-text behavior.
  import { COLLAPSIBLE_TEXT_LENGTH, COLLAPSIBLE_TEXT_LINE_COUNT } from '$lib/constants';
  import { ui } from '$lib/stores/ui.svelte';

  type Props = {
    text: string;
    storageKey: string;
    className?: string;
  };
  let { text, storageKey, className = 'hero-description' }: Props = $props();

  const normalized = $derived(text.trim());
  const lineCount = $derived(normalized.split(/\r\n|\r|\n/).length);
  const shouldCollapse = $derived(
    normalized.length > COLLAPSIBLE_TEXT_LENGTH || lineCount > COLLAPSIBLE_TEXT_LINE_COUNT,
  );
  const isExpanded = $derived(ui.isExpanded(storageKey));
  const stateClass = $derived(shouldCollapse && !isExpanded ? 'is-collapsed' : '');
</script>

{#if normalized}
  <div class="collapsible-text {className} {stateClass}">{normalized}</div>
  {#if shouldCollapse}
    <button
      type="button"
      class="text-toggle-button"
      aria-expanded={isExpanded}
      onclick={() => ui.toggleText(storageKey)}
    >
      {isExpanded ? 'show less' : '... see more'}
    </button>
  {/if}
{/if}

<style>
  .collapsible-text.is-collapsed {
    display: -webkit-box;
    -webkit-line-clamp: 6;
    line-clamp: 6;
    -webkit-box-orient: vertical;
    overflow: hidden;
  }

  .text-toggle-button {
    background: transparent;
    box-shadow: none;
    color: #9ab1d1;
    padding: 0.2rem 0;
    font-size: 0.82rem;
  }

  .text-toggle-button:hover {
    transform: none;
    background: transparent;
    color: #dbe7ff;
  }
</style>
