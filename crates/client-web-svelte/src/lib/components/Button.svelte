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
