<script lang="ts">
  // UserAvatar — replaces renderUserAvatar() (../client-web/src/app/ui.ts:73-83).
  // Resolves profile_image_url via resolveApiUrl; falls back to the uppercased
  // first letter of the username on a #263f5f tile.
  import { resolveApiUrl, type BootstrapUser } from '$lib/api';

  type Props = { user: BootstrapUser; class?: string };
  let { user, class: className = '' }: Props = $props();

  const imageUrl = $derived(user.profile_image_url ? resolveApiUrl(user.profile_image_url) : undefined);
  const initial = $derived(user.username.trim().charAt(0).toUpperCase() || '?');
</script>

<span class="user-avatar {className}">
  {#if imageUrl}
    <img src={imageUrl} alt="" loading="lazy" />
  {:else}
    <span>{initial}</span>
  {/if}
</span>
