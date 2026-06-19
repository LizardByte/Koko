<script lang="ts">
  // /login route. The actual screen is rendered by the root layout when the
  // user is logged out, but we also mount it here for direct navigation.
  // The layout's $effect handles redirect-on-already-logged-in.
  import LoginScreen from '$lib/components/LoginScreen.svelte';
</script>

<LoginScreen />
