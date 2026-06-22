<script lang="ts">
  // Button — the design-system button with a label and/or icon. Renders a
  // <button> by default, or an <a> when `href` is provided (so external links
  // can share the same visual variants without a separate "button-link"
  // affordance). Replaces renderButtonContent() (../client-web/src/app/ui.ts:
  // 60-71) + the vanilla `.button-link` class.
  //
  // For icon-only buttons (no label, fixed square size), use <IconButton>
  // instead — it's tailored for that case and avoids a Boolean prop here.
  //
  // Variant classes (.primary-button / .secondary-button / .danger-button /
  // .button-link) live in ./button.css — co-located with their only emitter.
  // SonarCloud's CSS analyzer does not scan .svelte files, so the
  // false-positive contrast warnings that fired on the global rules are gone.
  import Icon from './Icon.svelte';
  import './button.css';
  import type { Snippet } from 'svelte';
  import { buttonClasses, type ButtonVariant } from './button-types';

  type Props = {
    icon?: string;
    iconPosition?: 'start' | 'end';
    iconSize?: number;
    label?: string;
    variant?: ButtonVariant;
    busy?: boolean;
    disabled?: boolean;
    type?: 'button' | 'submit' | 'reset';
    id?: string;
    class?: string;
    title?: string;
    href?: string;
    target?: string;
    rel?: string;
    onclick?: (event: MouseEvent) => void;
    children?: Snippet;
    [key: `data-${string}`]: string | undefined;
  };

  let {
    icon,
    iconPosition = 'start',
    iconSize = 18,
    label,
    variant = 'primary',
    busy = false,
    disabled = false,
    type = 'button',
    id,
    class: className = '',
    title,
    href,
    target,
    rel,
    onclick,
    children,
    ...rest
  }: Props = $props();

  const classes = $derived(buttonClasses(variant, className));
</script>

{#if href}
  <a
    {href}
    {id}
    {title}
    {target}
    {rel}
    class="button-link {classes}"
    aria-busy={busy}
    aria-disabled={disabled || undefined}
    role={disabled ? 'link' : undefined}
    tabindex={disabled ? -1 : undefined}
    {onclick}
    {...rest}
  >
    {#if icon}<span class="button-icon"><Icon name={icon} size={iconSize} /></span>{/if}
    {#if label}<span class="button-label">{label}</span>{/if}
    {#if children}{@render children()}{/if}
  </a>
{:else}
  <button
    {type}
    {id}
    {title}
    {disabled}
    class={classes}
    class:is-busy={busy}
    aria-busy={busy}
    {onclick}
    {...rest}
  >
    {#if icon}<span class="button-icon"><Icon name={icon} size={iconSize} /></span>{/if}
    {#if label}<span class="button-label">{label}</span>{/if}
    {#if children}{@render children()}{/if}
  </button>
{/if}
