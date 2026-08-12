<script lang="ts">
  // WelcomeScreen — replaces renderWelcomeScreen() (../client-web/src/app/
  // auth.ts:80-95). Create-first-admin flow with username/password/pin/
  // birthday/profile-image. Profile image upload handled by
  // readProfileImageUpload semantics (validated client-side).
  import AuthShell from './AuthShell.svelte';
  import Button from './Button.svelte';
  import { auth, ui } from '$lib/stores';
  import { goto } from '$app/navigation';

  const ALLOWED_IMAGE_TYPES = new Set(['image/jpeg', 'image/png', 'image/webp', 'image/gif']);

  let username = $state('');
  let password = $state('');
  let pin = $state('');
  let birthday = $state('');
  let profileImage = $state<File | undefined>(undefined);
  let submitting = $state(false);

  async function readProfileImageUpload(): Promise<{ mime_type: string; data_base64: string } | undefined> {
    if (!profileImage || profileImage.size === 0) {
      return undefined;
    }
    if (!ALLOWED_IMAGE_TYPES.has(profileImage.type)) {
      throw new Error('Profile image must be a JPEG, PNG, WebP, or GIF file.');
    }
    if (profileImage.size > 2 * 1024 * 1024) {
      throw new Error('Profile image must be 2 MB or smaller.');
    }
    const dataUrl: string = await new Promise((resolve, reject) => {
      const reader = new FileReader();
      reader.addEventListener('load', () => resolve(typeof reader.result === 'string' ? reader.result : ''));
      reader.addEventListener('error', () => reject(new Error('Failed to read profile image.')));
      reader.readAsDataURL(profileImage!);
    });
    const dataBase64 = dataUrl.split(',')[1] ?? '';
    if (!dataBase64) {
      throw new Error('Failed to read profile image.');
    }
    return { mime_type: profileImage.type, data_base64: dataBase64 };
  }

  async function handleSubmit(event: SubmitEvent) {
    event.preventDefault();
    submitting = true;
    ui.clearError();
    try {
      const profileImageUpload = await readProfileImageUpload();
      await auth.createUser({
        username: username.trim(),
        password,
        pin: pin || undefined,
        admin: true,
        birthday: birthday || undefined,
        profile_image_upload: profileImageUpload,
        preferred_metadata_languages: ['en-US'],
      });
      await goto('/');
    } catch (err) {
      ui.setError(err instanceof Error ? err.message : String(err));
    } finally {
      submitting = false;
    }
  }
</script>

<AuthShell
  title="Create the first admin user"
  description="Koko needs one administrator account before the media library can be used."
>
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
        autocomplete="new-password"
        required
        bind:value={password}
      />
    </label>
    <label>
      Optional PIN
      <input
        name="pin"
        inputmode="numeric"
        pattern={'[0-9]{4,6}'}
        placeholder="1234"
        bind:value={pin}
      />
    </label>
    <label>
      Birthday
      <input name="birthday" type="date" bind:value={birthday} />
    </label>
    <label>
      Profile image
      <input
        name="profile_image_file"
        type="file"
        accept="image/png,image/jpeg,image/webp,image/gif"
        onchange={(event) => {
          const input = event.currentTarget as HTMLInputElement;
          profileImage = input.files?.[0];
        }}
      />
    </label>
    <Button type="submit" icon="user-plus" label="Create admin account" disabled={submitting} />
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
