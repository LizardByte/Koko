<script lang="ts">
  // Button — replaces renderButtonContent() (../client-web/src/app/ui.ts:60-71).
  // Reproduces the vanilla client's `.button-content` / `.button-icon` markup,
  // with optional icon at start or end. Children snippet allows custom content
  // when a plain label isn't enough.
  import Icon from './Icon.svelte';
  import type { Snippet } from 'svelte';

  type Props = {
    icon?: string;
    iconPosition?: 'start' | 'end';
    label?: string;
    variant?: 'primary' | 'secondary' | 'danger';
    busy?: boolean;
    disabled?: boolean;
    type?: 'button' | 'submit' | 'reset';
    id?: string;
    class?: string;
    title?: string;
    onclick?: (event: MouseEvent) => void;
    children?: Snippet;
    [key: `data-${string}`]: string | undefined;
  };

  let {
    icon,
    iconPosition = 'start',
    label,
    variant = 'primary',
    busy = false,
    disabled = false,
    type = 'button',
    id,
    class: className = '',
    title,
    onclick,
    children,
    ...rest
  }: Props = $props();

  const variantClass = $derived(
    variant === 'secondary' ? 'secondary-button' : variant === 'danger' ? 'danger-button' : '',
  );
</script>

<button
  {type}
  {id}
  {title}
  {disabled}
  class="{variantClass} {className}"
  class:is-busy={busy}
  aria-busy={busy}
  {onclick}
  {...rest}
>
  {#if icon}
    <span class="button-content" class:icon-end={iconPosition === 'end'}>
      <span class="button-icon"><Icon name={icon} size={16} strokeWidth={2.1} /></span>
      {#if children}{@render children()}{:else if label}<span>{label}</span>{/if}
    </span>
  {:else if children}
    {@render children()}
  {:else}
    {label}
  {/if}
</button>

<!--
  Busy spinner — component-scoped so the centering fix lives with the button,
  not in global app.css. Vanilla's style.css:83-98 has the same rule but its
  ::after spinner lacks centering (renders off-center to the right); we add
  `inset: 0; margin: auto` here so the spinner is centered. This is a
  documented delta — the global `button.is-busy` rule was removed from app.css.
  `@keyframes spin` stays global in app.css (shared with .loading-spinner).
-->
<style>
  .is-busy {
    position: relative;
    color: transparent;
    pointer-events: none;
  }

  .is-busy::after {
    content: '';
    position: absolute;
    /* Center the fixed-size spinner within the button. Vanilla omits this,
       leaving it off-center to the right. */
    inset: 0;
    margin: auto;
    width: 1rem;
    height: 1rem;
    border-radius: 999px;
    border: 2px solid rgba(255, 255, 255, 0.35);
    border-top-color: #fff;
    animation: spin 0.85s linear infinite;
  }
</style>
