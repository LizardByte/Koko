<script lang="ts">
  // LoginScreen — replaces renderLoginScreen() (../client-web/src/app/auth.ts:
  // 97-109). Uses bind:value + the auth store, no FormData rehydration.
  import AuthShell from './AuthShell.svelte';
  import Button from './Button.svelte';
  import { auth, ui } from '$lib/stores';
  import { goto } from '$app/navigation';

  let username = $state('');
  let password = $state('');
  let submitting = $state(false);

  async function handleSubmit(event: SubmitEvent) {
    event.preventDefault();
    submitting = true;
    ui.clearError();
    try {
      await auth.login({ username: username.trim(), password });
      await goto('/');
    } catch (err) {
      ui.setError(err instanceof Error ? err.message : String(err));
    } finally {
      submitting = false;
    }
  }
</script>

<AuthShell title="Sign in" description="Sign in with a Koko account to browse media and keep watch progress per user.">
  <form class="auth-form" onsubmit={handleSubmit}>
    <label>
      Username
      <input name="username" autocomplete="username" required bind:value={username} />
    </label>
    <label>
      Password
      <input
        name="password"
        type="password"
        autocomplete="current-password"
        required
        bind:value={password}
      />
    </label>
    <Button type="submit" icon="log-in" label="Sign in" disabled={submitting} />
  </form>
</AuthShell>

<style>
  .auth-form {
    display: flex;
    flex-direction: column;
    gap: 0.85rem;
  }
  .auth-form label {
    display: flex;
    flex-direction: column;
    gap: 0.45rem;
  }
</style>
