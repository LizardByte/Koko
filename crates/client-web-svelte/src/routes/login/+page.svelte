<script lang="ts">
  // Login page — replaces renderLoginScreen() (../client-web/src/app/auth.ts).
  // Demonstrates form handling with Svelte 5 bind:value (replacing the vanilla
  // client's FormData rehydration after each render) and the auth store.
  import { goto } from '$app/navigation';
  import { auth } from '$lib/auth.svelte';
  import Icon from '$lib/components/Icon.svelte';

  let username = $state('');
  let password = $state('');
  let error = $state<string | undefined>(undefined);
  let submitting = $state(false);

  async function handleSubmit(event: SubmitEvent) {
    event.preventDefault();
    submitting = true;
    error = undefined;
    try {
      await auth.login({ username: username.trim(), password });
      goto('/');
    } catch (err) {
      error = err instanceof Error ? err.message : String(err);
    } finally {
      submitting = false;
    }
  }
</script>

<svelte:head><title>Koko — Sign in</title></svelte:head>

<div class="login-shell">
  <form class="login-card" onsubmit={handleSubmit}>
    <div class="brand">
      <Icon name="house" size={28} /> Koko
    </div>
    <h1>Sign in</h1>
    <p class="muted">Welcome back. Sign in to continue.</p>

    {#if error}
      <div class="error"><Icon name="triangle-alert" size={16} /> {error}</div>
    {/if}

    <label>
      Username
      <input
        bind:value={username}
        autocomplete="username"
        required
        placeholder="admin"
      />
    </label>
    <label>
      Password
      <input
        type="password"
        bind:value={password}
        autocomplete="current-password"
        required
        placeholder="adminpass"
      />
    </label>

    <button type="submit" disabled={submitting}>
      {#if submitting}
        Signing in…
      {:else}
        <Icon name="log-in" size={16} /> Sign in
      {/if}
    </button>

    <div class="mock-hint muted">
      {#if import.meta.env.VITE_USE_MOCK_API === 'true'}
        Mock mode — use <code>admin</code> / <code>adminpass</code>.
      {/if}
    </div>
  </form>
</div>

<style>
  .login-shell {
    display: flex;
    align-items: center;
    justify-content: center;
    min-height: 70vh;
    padding: 2rem;
  }
  .login-card {
    width: 100%;
    max-width: 360px;
    background: var(--koko-surface, #fff);
    border: 1px solid var(--koko-border, #ddd);
    border-radius: 12px;
    padding: 2rem;
    box-shadow: 0 4px 24px rgba(0, 0, 0, 0.08);
  }
  .brand {
    display: inline-flex;
    align-items: center;
    gap: 0.4rem;
    font-weight: 700;
    font-size: 1.2rem;
    margin-bottom: 1.5rem;
  }
  h1 {
    margin: 0 0 0.3rem;
    font-size: 1.5rem;
  }
  .muted {
    color: var(--koko-muted, #777);
    font-size: 0.85rem;
    margin: 0 0 1.25rem;
  }
  .error {
    display: flex;
    align-items: center;
    gap: 0.4rem;
    background: rgba(239, 68, 68, 0.1);
    color: #b91c1c;
    padding: 0.6rem 0.8rem;
    border-radius: 6px;
    font-size: 0.85rem;
    margin-bottom: 1rem;
  }
  label {
    display: flex;
    flex-direction: column;
    gap: 0.3rem;
    font-size: 0.85rem;
    margin-bottom: 0.9rem;
  }
  input {
    padding: 0.5rem 0.6rem;
    border: 1px solid var(--koko-border, #ddd);
    border-radius: 6px;
    background: var(--koko-surface, #fff);
    color: inherit;
    font-size: 0.95rem;
  }
  button {
    width: 100%;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 0.4rem;
    padding: 0.6rem;
    border: none;
    border-radius: 6px;
    background: #2563eb;
    color: #fff;
    font-size: 0.95rem;
    font-weight: 600;
    cursor: pointer;
  }
  button:disabled {
    opacity: 0.6;
    cursor: not-allowed;
  }
  .mock-hint {
    margin-top: 1rem;
    text-align: center;
  }
  code {
    background: rgba(127, 127, 127, 0.15);
    padding: 0.05rem 0.3rem;
    border-radius: 3px;
    font-size: 0.85rem;
  }
</style>
